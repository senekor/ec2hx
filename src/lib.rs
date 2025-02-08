use std::{collections::BTreeMap, fmt::Write};

pub mod fmt;
pub mod parse;

pub static DEFAULT_LANGUAGES: &str = include_str!("../languages.toml");

#[derive(Debug, Clone)]
pub struct HelixLangCfg {
    name: String,
    indent: Option<(usize, IndentStyle)>,
    file_types: Option<Vec<String>>,
    has_formatter: bool,
    raw_toml: toml_edit::Table,
}

impl PartialEq for HelixLangCfg {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.indent == other.indent
            && self.file_types == other.file_types
            && self.has_formatter == other.has_formatter
    }
}

pub fn merge_languages(languages: &mut Vec<HelixLangCfg>, mut user_languages: Vec<HelixLangCfg>) {
    for cfg in languages.iter_mut() {
        let Some(user_lang_pos) = user_languages.iter().position(|l| l.name == cfg.name) else {
            continue;
        };
        let user_cfg = user_languages.remove(user_lang_pos);

        if let Some(indent) = user_cfg.indent {
            cfg.indent = Some(indent);
        }
        if let Some(file_types) = user_cfg.file_types {
            cfg.file_types = Some(file_types);
        }
        cfg.has_formatter |= user_cfg.has_formatter;

        for (k, v) in user_cfg.raw_toml {
            cfg.raw_toml.insert(k.as_str(), v);
        }
    }
    languages.extend(user_languages);
}

/// The returned tuple has the contents of config.toml and languages.toml.
pub fn ec2hx(
    languages: &[HelixLangCfg],
    input: &str,
    fallback_globs: Vec<String>,
    rulers: bool,
) -> (String, String) {
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

            // I previously thought it would be a good idea to not
            // generate overrides for each language in presence of a global
            // max_line_length config. It seemed redundant to me. However, we
            // still want the global editorconfig section to override default
            // or user configurations of per-language max_line_length. Therefore
            // we do need to generate this (mostly redundant) config for every
            // language.
            //
            // global_lang_cfg.max_line_length = None;

            continue;
        }

        // language-specific settings, use global values as default
        lang_cfg.with_defaults_from(&global_lang_cfg);

        'header_lang_loop: for lang in extract_langs_from_header(header) {
            let contains_glob_char = |l: &str| ["/", "**", "?"].iter().any(|c| l.contains(c));

            // homebrew example: [**.md] - this could just be [*.md] and
            // shouldn't be treated as a glob
            let lang_is_stupid_extension_glob = lang.starts_with("**.") && {
                let extension = lang.strip_prefix("**.").unwrap();
                !contains_glob_char(extension)
            };

            let basename = {
                let b = lang.rsplit_once('/').unwrap_or(("", &lang)).1;
                match b.rsplit_once("**") {
                    Some((_, n)) => format!("*{n}"),
                    None => b.into(),
                }
            };

            // We want to recognize "package.json" as json so we can copy its
            // config. However, we don't want to treat it exactly like json,
            // otherwise we'd override general "*.json" configuration.
            let basename_is_more_than_extension =
                basename.rsplit_once('.').is_some_and(|(pre, _)| pre != "*");

            let is_path_glob = !lang_is_stupid_extension_glob && contains_glob_char(&lang)
                || basename_is_more_than_extension;

            let ft_candidates = [
                basename
                    .rsplit_once('.')
                    .map(|(_, ext)| ext)
                    .unwrap_or(&basename)
                    .to_string(),
                basename,
            ];

            for supported_lang in languages {
                if supported_lang
                    .file_types
                    .iter()
                    .flatten()
                    .any(|ft| ft_candidates.contains(ft))
                {
                    let matched_name = supported_lang.name.to_string();
                    let mut lang_cfg = lang_cfg.clone();

                    // use potential previous matching section as default values,
                    // see for example ../test_data/python
                    if let Some(prev_lang_cfg) = hx_lang_cfg.get(&matched_name) {
                        lang_cfg.with_defaults_from(prev_lang_cfg);
                    }
                    // Use values from default languages.toml as default, for
                    // configurations where only size or style is specified.
                    // See for example ../test_data/cockroach where only
                    // indent_size is set in the global config.
                    lang_cfg.with_defaults_from_hx_config(supported_lang);

                    if supported_lang.has_formatter {
                        lang_cfg.trim_trailing_whitespace = Some(false);
                    }

                    if is_path_glob {
                        let name = make_synthetic_lang_name("glob", &lang);
                        let mut raw_toml = supported_lang.raw_toml.clone();
                        raw_toml.remove("injection-regex");
                        lang_cfg.raw_toml = Some(raw_toml);
                        lang_cfg.file_types = Some(vec![lang]);
                        hx_lang_cfg.insert(name, lang_cfg);
                    } else {
                        hx_lang_cfg.insert(matched_name, lang_cfg);
                    }
                    continue 'header_lang_loop;
                }
            }
            // The language in question doesn't seem to match any of
            // the languages supported by Helix. Probably the language
            // has neither an LSP nor a tree-sitter grammar, so there's
            // little reason to support it in Helix. Whatever the
            // reason may be, let's generate a custom language for
            // Helix with just the indent configuration.
            // An example for this situation: Linux Kconfig files
            let name = make_synthetic_lang_name("unknown", &lang);
            let mut lang_cfg = lang_cfg.clone();
            lang_cfg.file_types = Some(vec![lang]);

            // use potential previous matching section as default values,
            // see for example ../test_data/python
            if let Some(prev_lang_cfg) = hx_lang_cfg.get(&name) {
                lang_cfg.with_defaults_from(prev_lang_cfg);
            }

            hx_lang_cfg.insert(name, lang_cfg);
        }
    }

    let tab_langs_are_customized = global_lang_cfg.tab_width.is_some();
    let langs_without_formatters_are_customized =
        global_lang_cfg.trim_trailing_whitespace == Some(true);
    let all_langs_are_customized =
        global_lang_cfg.size.is_some() || global_lang_cfg.style.is_some();

    let languages_toml = if all_langs_are_customized
        || tab_langs_are_customized
        || langs_without_formatters_are_customized
    {
        let mut hx_global_lang_cfg = BTreeMap::new();
        for lang in languages {
            if hx_lang_cfg.contains_key(&lang.name) {
                continue;
            }
            if all_langs_are_customized
                || tab_langs_are_customized && matches!(lang.indent, Some((_, Tab)))
                || langs_without_formatters_are_customized && !lang.has_formatter
            {
                // language is eligible for customization
            } else {
                continue;
            }
            let mut lang_cfg = global_lang_cfg.clone();
            if lang.has_formatter {
                // Do not add formatter if language already has one.
                lang_cfg.trim_trailing_whitespace = Some(false);
            }
            lang_cfg.with_defaults_from_hx_config(lang);
            // I previously thought it would be a good idea to not generate
            // overrides for languages where the editorconfig already matches
            // the default or user helix config. However, if the user changes
            // their helix config, we still want editorconfig to take precedence
            // over that. So we do need to generate this (mostly redundant)
            // config for languages that are already configured that way.
            //
            // global_lang_cfg.max_line_length = None;
            // if lang_cfg.size == lang.cfg.size && lang_cfg.style == lang.cfg.style {
            //     // No need to override values that match the default
            //     lang_cfg.size = None;
            //     lang_cfg.style = None;
            // }
            hx_global_lang_cfg.insert(lang.name.clone(), lang_cfg);
        }

        // global fallback plain text language configuration
        let mut file_types = vec![];
        file_types.extend(fallback_globs);
        if !file_types.contains(&"*.txt".into()) {
            file_types.push("*.txt".into());
        }
        global_lang_cfg.file_types = Some(file_types);
        hx_global_lang_cfg.insert("ec2hx-global-fallback-plain-text".into(), global_lang_cfg);

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
                .map(|(name, cfg)| cfg.to_languages_toml(&name, rulers)),
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

fn make_synthetic_lang_name(kind: &str, lang: &str) -> String {
    let sanitized_glob = lang.replace(['/'], "-");
    format!("ec2hx-{kind}-lang-{sanitized_glob}")
}

fn extract_langs_from_header(header: &str) -> Vec<String> {
    if header.contains(['!', '\\']) || header.contains("..") {
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

    let mut res = stack.pop().unwrap();
    // Linux has a weird section header with a comma at the end
    res.retain(|l| !l.is_empty());
    res
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

#[derive(Debug, Clone, Default)]
pub struct LangCfg {
    size: Option<usize>,
    style: Option<IndentStyle>,
    tab_width: Option<usize>,
    max_line_length: Option<usize>,
    trim_trailing_whitespace: Option<bool>,
    // not part of editorconfig, used to generate custom configs for languages
    // unsupported by Helix
    file_types: Option<Vec<String>>,
    // not part of editorconfig, used to generate custom configs for synthetic
    // languages used to support arbitrary path globs
    raw_toml: Option<toml_edit::Table>,
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
        if self.trim_trailing_whitespace.is_none() {
            self.trim_trailing_whitespace = other.trim_trailing_whitespace;
        }
        self
    }

    /// This adds some defaults to the language configuration based on the Helix
    /// configuration. It's more conservative than [Self::with_defaults_from],
    /// because we only want to fill in the gaps of an indent configuration and
    /// not generate unnecessary overrides that match the Helix config anyway.
    fn with_defaults_from_hx_config(&mut self, other: &HelixLangCfg) -> &mut Self {
        let Some((other_size, other_style)) = other.indent else {
            return self;
        };
        if self.tab_width.is_some() && self.style != Some(Space) {
            if self.style == Some(Tab) {
                // Do nothing. indent_size has precedence over
                // tab_width if they both come from .editorconfig,
                // but we don't want to override an editorconfig's
                // tab_width with an indent_size from the default
                // languages.toml.
            } else {
                match other_style {
                    Tab => self.style = Some(Tab),
                    Space => {
                        // self.style is none and other style is space.
                        // tab_width will be overruled.
                        self.style = Some(Space);
                        if self.size.is_none() {
                            self.size = Some(other_size);
                        }
                    }
                }
            }
        } else if self.style.is_some() || self.size.is_some() {
            // Use values from default languages.toml as default, for
            // configurations where only size or style is specified.
            // See for example ../test_data/cockroach where only
            // indent_size if set in the global config.
            if self.size.is_none() {
                self.size = Some(other_size)
            }
            if self.style.is_none() {
                self.style = Some(other_style)
            }
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
        let trim_trailing_whitespace = section
            .get(&Key::TrimTrailingWhitespace)
            .and_then(|s| s.parse().ok());
        Self {
            size,
            style,
            tab_width,
            max_line_length,
            trim_trailing_whitespace,
            file_types: None,
            raw_toml: None,
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
        if indent.is_none()
            && self.max_line_length.is_none()
            && self.trim_trailing_whitespace != Some(true)
        {
            return String::new();
        }

        let mut t = match self.raw_toml.clone() {
            Some(t) => t,
            None => toml_edit::Table::new(),
        };

        t.insert("name", lang.into());

        if let Some(file_types) = self.file_types.as_ref() {
            if self.raw_toml.is_none() {
                // language is unsupported by Helix, generate necessary config
                // for custom language
                t.insert("scope", "text.plain".into());
            }

            let file_types = file_types
                .iter()
                .map(|ft| {
                    let mut m = toml_edit::InlineTable::new();
                    m.insert("glob", ft.as_str().into());
                    m
                })
                .collect::<toml_edit::Array>()
                .into();
            t.insert("file-types", file_types);
        }

        if let Some((unit, tab_width)) = indent {
            let mut m = toml_edit::InlineTable::new();
            m.insert("unit", unit.into());
            m.insert("tab-width", (tab_width as i64).into());
            t.insert("indent", m.into());
        }

        if let Some(max_line_length) = self.max_line_length {
            t.insert("text-width", (max_line_length as i64).into());
            if rulers {
                let len: toml_edit::Value = ((max_line_length + 1) as i64).into();
                let array: toml_edit::Array = [len].into_iter().collect();
                t.insert("rulers", array.into());
            }
        }

        if let Some(true) = self.trim_trailing_whitespace {
            let mut m = toml_edit::InlineTable::new();
            m.insert("command", "ec2hx".into());
            let arg = toml_edit::Value::from("trim-trailing-whitespace");
            let args: toml_edit::Array = [arg].into_iter().collect();
            m.insert("args", args.into());
            t.insert("formatter", m.into());
            t.insert("auto-format", true.into());
        }

        format!("[[language]]\n{t}\n")
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

    let actual = extract_langs_from_header("tools/{perf,power,rcu,testing/kunit}/**.py");
    let expected = vec![
        "tools/perf/**.py",
        "tools/power/**.py",
        "tools/rcu/**.py",
        "tools/testing/kunit/**.py",
    ];
    assert_eq!(actual, expected);
}

#[test]
fn snapshot() {
    let languages = parse::languages(DEFAULT_LANGUAGES);
    insta::glob!("..", "test_data/*", |path| {
        let input = std::fs::read_to_string(path).unwrap();
        let (config_toml, languages_toml) = ec2hx(&languages, &input, vec!["*.foo".into()], false);
        insta::assert_snapshot!("conf", config_toml);
        insta::assert_snapshot!("lang", languages_toml);
    });
}

#[test]
fn rulers() {
    let languages = parse::languages(DEFAULT_LANGUAGES);
    // global rulers
    let input = std::fs::read_to_string("test_data/webpack").unwrap();
    let (config_toml, _) = ec2hx(&languages, &input, vec![], true);
    insta::assert_snapshot!("rulers-conf", config_toml);
    // language rulers
    let input = std::fs::read_to_string("test_data/php").unwrap();
    let (_, languages_toml) = ec2hx(&languages, &input, vec![], true);
    insta::assert_snapshot!("rulers-lang", languages_toml);
}

#[test]
fn merge_langs() {
    let mut languages = vec![
        HelixLangCfg {
            name: "unchanged".into(),
            indent: Some((2, Space)),
            file_types: Some(vec!["*.unchanged".into()]),
            has_formatter: false,
            raw_toml: toml_edit::Table::new(),
        },
        HelixLangCfg {
            name: "partial".into(),
            indent: None,
            file_types: Some(vec!["*.partial".into()]),
            has_formatter: true,
            raw_toml: toml_edit::Table::new(),
        },
    ];
    let user_languages = vec![
        HelixLangCfg {
            name: "partial".into(),
            indent: Some((4, Space)),
            file_types: Some(vec!["*.partial".into(), "*.partial.local".into()]),
            has_formatter: false,
            raw_toml: toml_edit::Table::new(),
        },
        HelixLangCfg {
            name: "new".into(),
            indent: Some((3, Tab)),
            file_types: Some(vec!["*.new".into()]),
            has_formatter: false,
            raw_toml: toml_edit::Table::new(),
        },
    ];
    let expected = vec![
        HelixLangCfg {
            name: "unchanged".into(),
            indent: Some((2, Space)),
            file_types: Some(vec!["*.unchanged".into()]),
            has_formatter: false,
            raw_toml: toml_edit::Table::new(),
        },
        HelixLangCfg {
            name: "partial".into(),
            indent: Some((4, Space)),
            file_types: Some(vec!["*.partial".into(), "*.partial.local".into()]),
            has_formatter: true,
            raw_toml: toml_edit::Table::new(),
        },
        HelixLangCfg {
            name: "new".into(),
            indent: Some((3, Tab)),
            file_types: Some(vec!["*.new".into()]),
            has_formatter: false,
            raw_toml: toml_edit::Table::new(),
        },
    ];
    merge_languages(&mut languages, user_languages);

    assert_eq!(languages, expected);
}
