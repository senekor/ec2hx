use std::{collections::BTreeMap, fmt::Write};

pub mod lang {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub(crate) struct Language {
        pub(crate) name: &'static str,
        pub(crate) file_types: &'static [&'static str],
    }

    // A custom language as fallback for plain text files where global indent
    // configuration should apply as well. Hardcoded - exentd as users report
    // what they need.
    pub(crate) static PLAIN_TEXT: Language = Language {
        name: "ec2hx-global-fallback-plain-text",
        file_types: &["*.txt"],
    };

    /// Language name-to-file-types pairings parsed from Helix' default
    /// languages.toml. See this crate's build script.
    pub(crate) static LANGUAGES: &[Language] = include!(concat!(env!("OUT_DIR"), "/lang.rs"));
}

/// The returned tuple has the contents of config.toml and languages.toml.
pub fn ec2hx(input: &str) -> (String, String) {
    let mut editorconfig = EditorConfig::from(input);

    // don't care about preample (usually just "root = true")
    let (_, _preample) = editorconfig.sections.remove(0);

    let mut global_indent_size: Option<&str> = None;
    let mut global_indent_style: Option<&str> = None;

    let mut hx_editor_cfg = HxEditorCfg::default();
    let mut hx_global_lang_cfg = None;
    let mut hx_lang_cfg = BTreeMap::<String, HxIndentCfg>::new();

    for (header, section) in editorconfig.sections {
        if header == "*" {
            // apply global editor settings
            hx_editor_cfg = HxEditorCfg::from(&section);
            // remember global defaults for language-specific stuff
            global_indent_size = section.get(&Key::IndentSize).cloned();
            global_indent_style = section.get(&Key::IndentStyle).cloned();
        }

        let Some(indent) =
            HxIndentCfg::try_from(&section, &global_indent_size, &global_indent_style)
        else {
            continue;
        };

        if header == "*" {
            // track globally-defined language settings separately
            hx_global_lang_cfg = Some(indent);
        } else {
            for lang in extract_langs_from_header(header) {
                let mut short_lang = lang.as_str();
                short_lang = short_lang.strip_prefix("*.").unwrap_or(short_lang);

                let mut found_match = false;
                for supported_lang in lang::LANGUAGES {
                    if supported_lang.file_types.iter().any(|ft| *ft == short_lang) {
                        found_match = true;
                        hx_lang_cfg.insert(supported_lang.name.into(), indent.clone());
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
                    let mut indent = indent.clone();
                    indent.file_types.push(lang);

                    hx_lang_cfg.insert(name, indent);
                }
            }
        }
    }

    let languages_toml = match hx_global_lang_cfg {
        Some(mut indent) => {
            let mut hx_global_lang_cfg = BTreeMap::new();
            for lang in lang::LANGUAGES {
                hx_global_lang_cfg.insert(lang.name, indent.clone());
            }

            // global fallback plain text language configuration
            indent
                .file_types
                .extend(lang::PLAIN_TEXT.file_types.iter().map(|s| s.to_string()));
            hx_global_lang_cfg.insert(lang::PLAIN_TEXT.name, indent);

            // to not apply global settings to languages with overrides
            for lang in hx_lang_cfg.keys() {
                hx_global_lang_cfg.remove(lang.as_str());
            }

            format!(
                "\
# language-specific settings:

{}\
################################################################################

# global settings, applied equally to all remaining languages:

{}",
                hx_lang_cfg.to_string(),
                hx_global_lang_cfg.to_string(),
            )
        }
        None => hx_lang_cfg.to_string(),
    };

    (hx_editor_cfg.to_string(), languages_toml)
}

fn extract_langs_from_header(header: &str) -> Vec<String> {
    if header.contains(['/', '?', '[', ']', '!']) || header.contains("**") || header.contains("..")
    {
        // deranged section detected, give up
        return Vec::new();
    }

    let mut rest = header;
    let mut stack = vec![vec![]];
    let mut should_expanded_at_next_delimiter = false;

    'outer: loop {
        for i in 0..=rest.len() {
            if i == rest.len() || matches!(rest.as_bytes()[i], b'{' | b',' | b'}') {
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
}

impl HxEditorCfg {
    fn from(section: &BTreeMap<Key, &str>) -> Self {
        let mut res = Self::default();
        if let Some(line_ending) = section.get(&Key::EndOfLine) {
            let line_ending = line_ending.to_lowercase();
            match line_ending.as_str() {
                "lf" => res.default_line_ending = Some("lf"),
                "crlf" => res.default_line_ending = Some("crlf"),
                _ => {}
            }
        }
        if let Some(insert_final_newline) = section.get(&Key::InsertFinalNewline) {
            let insert_final_newline = insert_final_newline.to_lowercase();
            match insert_final_newline.as_str() {
                "true" => res.insert_final_newline = Some(true),
                "false" => res.insert_final_newline = Some(false),
                _ => {}
            }
        }
        res
    }
}

impl std::fmt::Display for HxEditorCfg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(default_line_ending) = self.default_line_ending {
            writeln!(f, "editor.default-line-ending = {default_line_ending:?}")?;
        }
        if let Some(insert_final_newline) = self.insert_final_newline {
            writeln!(f, "editor.insert-final-newline = {insert_final_newline}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct HxIndentCfg {
    unit: String,
    tab_width: usize,
    /// to generate custom configs for languages unsupported by Helix
    file_types: Vec<String>,
}

impl HxIndentCfg {
    fn try_from<'a>(
        section: &BTreeMap<Key, &'a str>,
        global_indent_size: &Option<&'a str>,
        global_indent_style: &Option<&'a str>,
    ) -> Option<Self> {
        let indent_size = section
            .get(&Key::IndentSize)
            .or(global_indent_size.as_ref())?;
        let indent_style = section
            .get(&Key::IndentStyle)
            .or(global_indent_style.as_ref())?;

        let indent_size = indent_size
            .parse::<usize>()
            .expect("indent_size must be a whole number");

        let unit = {
            let indent_style = indent_style.to_lowercase();
            match indent_style.as_str() {
                "tab" => "\t".into(),
                "space" => " ".repeat(indent_size),
                _ => return None,
            }
        };

        Some(HxIndentCfg {
            unit,
            tab_width: indent_size,
            file_types: Vec::new(),
        })
    }
}

trait HxIndentCfgExt {
    fn to_string(&self) -> String;
}
impl<T: AsRef<str>> HxIndentCfgExt for BTreeMap<T, HxIndentCfg> {
    fn to_string(&self) -> String {
        let mut f = String::new();
        for (lang, indent) in self.iter() {
            let lang = lang.as_ref();
            f.push_str("[[language]]\n");
            writeln!(f, "name = {lang:?}").unwrap();

            let unit = &indent.unit;
            let tab_width = indent.tab_width;

            if !indent.file_types.is_empty() {
                // language is unsupported by Helix, generate necessary config
                // for custom language
                writeln!(f, r#"scope = "text.plain""#).unwrap();
                write!(f, "file-types = [").unwrap();
                for ft in indent.file_types.iter() {
                    write!(f, "{{ glob = {ft:?} }},").unwrap();
                }
                writeln!(f, "]").unwrap();
            }

            f.push_str(&format!(
                "indent = {{ unit = {unit:?}, tab-width = {tab_width} }}\n\n"
            ));
        }
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
        let (config_toml, languages_toml) = ec2hx(&input);
        insta::assert_snapshot!("conf", config_toml);
        insta::assert_snapshot!("lang", languages_toml);
    });
}
