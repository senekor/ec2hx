use std::collections::BTreeMap;

// obtained with:
// hx --health languages | tail --lines +2 | awk '{ print $1 }' | wl-copy
// TODO: fetch this dynamically to stay up to date?
const SUPPORTED_LANGS: &str = include_str!("supported_langs.txt");

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
                hx_lang_cfg.insert(lang, indent.clone());
            }
        }
    }

    let languages_toml = match hx_global_lang_cfg {
        Some(indent) => {
            let mut hx_global_lang_cfg = BTreeMap::new();
            for lang in SUPPORTED_LANGS.lines() {
                hx_global_lang_cfg.insert(lang, indent.clone());
            }
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
                cur_section.insert(key, value);
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
            f.push_str(&format!("name = {lang:?}\n"));

            let unit = &indent.unit;
            let tab_width = indent.tab_width;

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
