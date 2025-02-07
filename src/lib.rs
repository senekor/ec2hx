use std::{collections::BTreeMap, fmt::Write};

pub mod lang {
    use crate::*;

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub(crate) struct Language {
        pub(crate) name: &'static str,
        pub(crate) file_types: &'static [&'static str],
        pub(crate) cfg: LangCfg,
    }

    /// Language name-to-file-types pairings parsed from Helix' default
    /// languages.toml. See this crate's build script.
    pub(crate) static LANGUAGES: &[Language] = include!(concat!(env!("OUT_DIR"), "/lang.rs"));
}

/// The returned tuple has the contents of config.toml and languages.toml.
pub fn ec2hx(input: &str, fallback_globs: Vec<String>, rulers: bool) -> (String, String) {
    let mut editorconfig = EditorConfig::from(input);

    // don't care about preample (usually just "root = true")
    let (_, _preample) = editorconfig.sections.remove(0);

    let mut global_lang_cfg = LangCfg::default();

    let mut hx_editor_cfg = HxEditorCfg::default();
    let mut hx_lang_cfg = BTreeMap::<String, LangCfg>::new();

    for (header, section) in editorconfig.sections {
        let mut lang_cfg = LangCfg::from(&section);

        if header == "*" {
            // apply global editor settings
            hx_editor_cfg = HxEditorCfg::from(&section);

            // remember global defaults for language-specific stuff
            global_lang_cfg = lang_cfg;

            // already configured globally, no need to override for each lang
            global_lang_cfg.max_line_length = None;

            continue;
        }

        // language-specific settings, use global values as default
        lang_cfg.with_defaults_from(&global_lang_cfg);

        for lang in extract_langs_from_header(header) {
            let mut short_lang = lang.as_str();
            short_lang = short_lang.strip_prefix("*.").unwrap_or(short_lang);

            let mut found_match = false;
            for supported_lang in lang::LANGUAGES {
                if supported_lang
                    .file_types
                    .iter()
                    .any(|ft| *ft == short_lang || *ft == lang)
                {
                    found_match = true;
                    let matched_name = supported_lang.name.into();
                    let mut indent_cfg = lang_cfg.clone();

                    // use potential previous matching section as default values,
                    // see for example ../test_data/python
                    if let Some(prev_indent_cfg) = hx_lang_cfg.get(&matched_name) {
                        indent_cfg.with_defaults_from(prev_indent_cfg);
                    }
                    // Use values from default languages.toml as default, for
                    // configurations where only size or style is specified.
                    // See for example ../test_data/cockroach where only
                    // indent_size is set in the global config.
                    indent_cfg.with_defaults_respecting_tab_width(&supported_lang.cfg);

                    hx_lang_cfg.insert(matched_name, indent_cfg);
                    break;
                }
            }
            if !found_match {
                // The language in question doesn't seem to match any of
                // the languages supported by Helix. Probably the language
                // has neither an LSP nor a tree-sitter grammar, so there's
                // little reason to support it in Helix. Whatever the
                // reason may be, let's generate a custom language for
                // Helix with just the indent configuration.
                // An example for this situation: Linux Kconfig files
                let name = format!("ec2hx-unknown-lang-{short_lang}");
                let mut indent_cfg = lang_cfg.clone();
                indent_cfg.file_types.push(lang);

                // use potential previous matching section as default values,
                // see for example ../test_data/python
                if let Some(prev_indent_cfg) = hx_lang_cfg.get(&name) {
                    indent_cfg.with_defaults_from(prev_indent_cfg);
                }

                hx_lang_cfg.insert(name, indent_cfg);
            }
        }
    }

    let tab_langs_are_customized = global_lang_cfg.tab_width.is_some();
    let all_langs_are_customized =
        global_lang_cfg.size.is_some() || global_lang_cfg.style.is_some();

    let languages_toml = if all_langs_are_customized || tab_langs_are_customized {
        let mut hx_global_lang_cfg = BTreeMap::new();
        for lang in lang::LANGUAGES {
            if hx_lang_cfg.contains_key(lang.name) {
                continue;
            }
            if lang.cfg.style != Some(Tab) && !all_langs_are_customized {
                continue;
            }
            let mut indent_cfg = global_lang_cfg.clone();
            indent_cfg.with_defaults_respecting_tab_width(&lang.cfg);
            hx_global_lang_cfg.insert(lang.name, indent_cfg);
        }

        // global fallback plain text language configuration
        global_lang_cfg.file_types.extend(fallback_globs);
        if !global_lang_cfg.file_types.contains(&"*.txt".into()) {
            global_lang_cfg.file_types.push("*.txt".into());
        }
        hx_global_lang_cfg.insert("ec2hx-global-fallback-plain-text", global_lang_cfg);

        ["\
# language-specific settings:

"
        .to_string()]
        .into_iter()
        .chain(
            hx_lang_cfg
                .into_iter()
                .map(|(name, cfg)| cfg.to_languages_toml(&name, rulers)),
        )
        .chain(["\
################################################################################

# global settings, applied equally to all remaining languages:

"
        .to_string()])
        .chain(
            hx_global_lang_cfg
                .into_iter()
                .map(|(name, cfg)| cfg.to_languages_toml(name, rulers)),
        )
        .collect()
    } else {
        hx_lang_cfg
            .into_iter()
            .map(|(name, cfg)| cfg.to_languages_toml(&name, rulers))
            .collect()
    };

    (hx_editor_cfg.to_config_toml(rulers), languages_toml)
}

fn extract_langs_from_header(header: &str) -> Vec<String> {
    if header.contains(['/', '?', '!', '\\']) || header.contains("**") || header.contains("..") {
        // deranged section detected, give up
        return Vec::new();
    }

    let mut rest = header;
    let mut stack = vec![vec![]];
    let mut should_expanded_at_next_delimiter = false;

    'outer: loop {
        for i in 0..=rest.len() {
            if i == rest.len() || b"{,}[".contains(&rest.as_bytes()[i]) {
                if should_expanded_at_next_delimiter {
                    should_expanded_at_next_delimiter = false;
                    let fragments = stack.pop().unwrap();
                    let current_buffer = stack.last_mut().unwrap();
                    let prefix = current_buffer.pop().unwrap_or_default();
                    let suffix = rest[0..i].to_string();

                    for frag in fragments {
                        current_buffer.push(format!("{prefix}{frag}{suffix}"));
                    }
                } else {
                    stack.last_mut().unwrap().push(rest[0..i].to_string());
                }
                match rest.as_bytes().get(i) {
                    Some(b'{') => stack.push(Vec::new()), // recurse deeper
                    Some(b'}') => should_expanded_at_next_delimiter = true,
                    Some(b'[') => {
                        // process charset
                        let mut charset = Vec::new();
                        rest = &rest[i + 1..];
                        while rest.as_bytes()[0] != b']' {
                            charset.extend(rest.chars().take(1).map(String::from));
                            rest = &rest[1..];
                        }
                        stack.push(charset);
                        should_expanded_at_next_delimiter = true;
                        continue 'outer;
                    }
                    None => break 'outer,
                    _ => {} // b','
                }
                rest = &rest[i + 1..];
                continue 'outer; // make sure counter starts back at 0
            }
        }
    }

    stack.pop().unwrap()
}

#[derive(Debug, Clone, Default)]
struct EditorConfig<'a> {
    // first section is implicitly the preample
    sections: Vec<(&'a str, BTreeMap<Key, &'a str>)>,
}

/// see https://github.com/editorconfig/editorconfig/wiki/EditorConfig-Properties
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Key {
    IndentStyle,
    IndentSize,
    TabWidth,
    EndOfLine,
    Charset,
    SpellingLanguage,
    TrimTrailingWhitespace,
    InsertFinalNewline,
    Root,
    MaxLineLength,
    Unknown,
}

impl From<&str> for Key {
    fn from(value: &str) -> Self {
        use Key::*;
        match value.to_lowercase().as_str() {
            "indent_style" => IndentStyle,
            "indent_size" => IndentSize,
            "tab_width" => TabWidth,
            "end_of_line" => EndOfLine,
            "charset" => Charset,
            "spelling_language" => SpellingLanguage,
            "trim_trailing_whitespace" => TrimTrailingWhitespace,
            "insert_final_newline" => InsertFinalNewline,
            "root" => Root,
            "max_line_length" => MaxLineLength,
            _ => Unknown,
        }
    }
}

impl<'a> EditorConfig<'a> {
    fn from(input: &'a str) -> Self {
        let mut res = Self::default();
        let mut cur_section_header = "";
        let mut cur_section = BTreeMap::new();

        for line in input.lines() {
            let line = line.trim();
            if line.is_empty() || line.is_comment() {
                continue;
            }

            if let Some(header) = line.try_parse_section_header() {
                res.sections.push((cur_section_header, cur_section));
                cur_section_header = header;
                cur_section = BTreeMap::new();
            } else {
                let (key, value) = line.parse_key_value_pair();
                // see: ../test_data/pandoc and ../test_data/nodejs
                // I guess an empty value means "unsetting" a global config.
                // This is not really relevant to us, because global configs
                // aren't applied to zip, docx etc. anyway...
                if !matches!(value, "" | "unset" | "ignore") {
                    cur_section.insert(key, value);
                }
            }
        }
        res.sections.push((cur_section_header, cur_section));

        res
    }
}

/// see: https://spec.editorconfig.org/#file-format
trait EditorConfigStrExtension {
    fn is_comment(&self) -> bool;
    fn try_parse_section_header(&self) -> Option<&str>;
    fn parse_key_value_pair(&self) -> (Key, &str);
}

impl EditorConfigStrExtension for str {
    fn is_comment(&self) -> bool {
        self.starts_with([';', '#'])
    }

    fn try_parse_section_header(&self) -> Option<&str> {
        if self.starts_with('[') && self.ends_with(']') {
            Some(&self[1..self.len() - 1])
        } else {
            None
        }
    }

    fn parse_key_value_pair(&self) -> (Key, &str) {
        let (key, value) = self.split_once('=').expect("invalid key-value pair");

        (Key::from(key.trim()), value.trim())
    }
}

#[derive(Debug, Clone, Default)]
pub struct HxEditorCfg {
    default_line_ending: Option<&'static str>,
    insert_final_newline: Option<bool>,
    max_line_length: Option<usize>,
}

impl HxEditorCfg {
    fn from(section: &BTreeMap<Key, &str>) -> Self {
        let default_line_ending = section.get(&Key::EndOfLine).and_then(|s| match *s {
            "lf" => Some("lf"),
            "crlf" => Some("crlf"),
            _ => None,
        });
        let insert_final_newline = section
            .get(&Key::InsertFinalNewline)
            .and_then(|s| s.parse().ok());
        let max_line_length = section
            .get(&Key::MaxLineLength)
            .and_then(|s| s.parse().ok());
        Self {
            default_line_ending,
            insert_final_newline,
            max_line_length,
        }
    }

    fn to_config_toml(&self, rulers: bool) -> String {
        let mut f = String::new();
        if let Some(default_line_ending) = self.default_line_ending {
            writeln!(f, "editor.default-line-ending = {default_line_ending:?}").unwrap();
        }
        if let Some(insert_final_newline) = self.insert_final_newline {
            writeln!(f, "editor.insert-final-newline = {insert_final_newline}").unwrap();
        }
        if let Some(max_line_length) = self.max_line_length {
            writeln!(f, "editor.text-width = {max_line_length}").unwrap();
            if rulers {
                writeln!(f, "editor.rulers = [{}]", max_line_length + 1).unwrap();
            }
        }
        f
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum IndentStyle {
    Space,
    Tab,
}
use IndentStyle::*;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct LangCfg {
    size: Option<usize>,
    style: Option<IndentStyle>,
    tab_width: Option<usize>,
    max_line_length: Option<usize>,
    /// to generate custom configs for languages unsupported by Helix
    file_types: Vec<String>,
}

impl LangCfg {
    fn parse_style(style: &&str) -> Option<IndentStyle> {
        match style.to_lowercase().as_str() {
            "tab" => Some(Tab),
            "space" => Some(Space),
            _ => None,
        }
    }

    fn with_defaults_from(&mut self, other: &Self) -> &mut Self {
        if self.size.is_none() {
            self.size = other.size
        }
        if self.style.is_none() {
            self.style = other.style
        }
        if self.tab_width.is_none() {
            self.tab_width = other.tab_width
        }
        if self.max_line_length.is_none() {
            self.max_line_length = other.max_line_length;
        }
        self
    }

    fn with_defaults_respecting_tab_width(&mut self, other: &Self) -> &mut Self {
        if self.tab_width.is_some() && self.style != Some(Space) {
            if self.style == Some(Tab) {
                // Do nothing. indent_size has precedence over
                // tab_width if they both come from .editorconfig,
                // but we don't want to override an editorconfig's
                // tab_width with an indent_size from the default
                // languages.toml.
            } else {
                match other.style {
                    Some(Tab) => self.style = Some(Tab),
                    Some(Space) => {
                        // self.style is none and other style is space.
                        // tab_width will be overruled.
                        self.with_defaults_from(other);
                    }
                    None => {}
                }
            }
        } else {
            // Use values from default languages.toml as default, for
            // configurations where only on size or style is specified.
            // See for example ../test_data/cockroach where only
            // indent_size if set in the global config.
            self.with_defaults_from(other);
        }
        if self.max_line_length.is_none() {
            self.max_line_length = other.max_line_length;
        }
        self
    }

    fn from(section: &BTreeMap<Key, &str>) -> Self {
        let size = section.get(&Key::IndentSize).and_then(|s| s.parse().ok());
        let tab_width = section.get(&Key::TabWidth).and_then(|s| s.parse().ok());
        let style = section.get(&Key::IndentStyle).and_then(Self::parse_style);
        let max_line_length = section
            .get(&Key::MaxLineLength)
            .and_then(|s| s.parse().ok());
        Self {
            size,
            style,
            tab_width,
            max_line_length,
            file_types: vec![],
        }
    }

    fn to_languages_toml(&self, lang: &str, rulers: bool) -> String {
        let indent = 'indent: {
            let Some(indent_style) = self.style else {
                break 'indent None;
            };
            match (indent_style, self.size, self.tab_width) {
                (Space, Some(size), _) => Some((" ".repeat(size), size)), // tab_width doesn't affect space
                (Tab, Some(size), _) | (Tab, None, Some(size)) => Some(("\t".into(), size)),
                (Space, None, _) | (Tab, None, None) => None,
            }
        };
        if indent.is_none() && self.max_line_length.is_none() {
            return String::new();
        }

        let mut f = String::new();

        f.push_str("[[language]]\n");
        writeln!(f, "name = {lang:?}").unwrap();

        if !self.file_types.is_empty() {
            // language is unsupported by Helix, generate necessary config
            // for custom language
            writeln!(f, r#"scope = "text.plain""#).unwrap();
            write!(f, "file-types = [").unwrap();
            for ft in self.file_types.iter() {
                write!(f, "{{ glob = {ft:?} }},").unwrap();
            }
            writeln!(f, "]").unwrap();
        }

        if let Some((unit, tab_width)) = indent {
            writeln!(f, "indent = {{ unit = {unit:?}, tab-width = {tab_width} }}").unwrap();
        }

        if let Some(max_line_length) = self.max_line_length {
            writeln!(f, "text-width = {max_line_length}").unwrap();
            if rulers {
                writeln!(f, "rulers = [{}]", max_line_length + 1).unwrap();
            }
        }

        f.push('\n');
        f
    }
}

#[test]
fn extract_langs() {
    let actual = extract_langs_from_header("Makefile");
    let expected = vec!["Makefile"];
    assert_eq!(actual, expected);

    let actual = extract_langs_from_header("*.{json,js}");
    let expected = vec!["*.json", "*.js"];
    assert_eq!(actual, expected);

    let actual =
        extract_langs_from_header("{*.{awk,c,dts,dtsi,dtso,h,mk,s,S},Kconfig,Makefile,Makefile.*}");
    let expected = vec![
        "*.awk",
        "*.c",
        "*.dts",
        "*.dtsi",
        "*.dtso",
        "*.h",
        "*.mk",
        "*.s",
        "*.S",
        "Kconfig",
        "Makefile",
        "Makefile.*",
    ];
    assert_eq!(actual, expected);
}

#[test]
fn snapshot() {
    insta::glob!("..", "test_data/*", |path| {
        let input = std::fs::read_to_string(path).unwrap();
        let (config_toml, languages_toml) = ec2hx(&input, vec!["*.foo".into()], false);
        insta::assert_snapshot!("conf", config_toml);
        insta::assert_snapshot!("lang", languages_toml);
    });
}

#[test]
fn rulers() {
    // global rulers
    let input = std::fs::read_to_string("test_data/webpack").unwrap();
    let (config_toml, _) = ec2hx(&input, vec![], true);
    insta::assert_snapshot!("conf", config_toml);
    // language rulers
    let input = std::fs::read_to_string("test_data/php").unwrap();
    let (_, languages_toml) = ec2hx(&input, vec![], true);
    insta::assert_snapshot!("lang", languages_toml);
}
