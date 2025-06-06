use std::{collections::BTreeMap, fmt::Write, str::FromStr};

pub mod fmt;
pub mod parse;

pub static DEFAULT_LANGUAGES: &str = include_str!("../languages.toml");

#[derive(Debug, Clone)]
pub struct HelixLangCfg {
    name: String,
    indent: Option<(usize, IndentStyle)>,
    file_types: Option<Vec<FileType>>,
    has_formatter: bool,
    raw_toml: toml_edit::Table,
}

impl HelixLangCfg {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_has_formatter(&mut self, val: bool) {
        self.has_formatter = val;
    }
}

#[derive(Debug, Clone, PartialEq)]
enum FileType {
    Extension(String),
    Glob(String),
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
) -> (String, String, BTreeMap<String, String>) {
    let fallback_globs = {
        let mut fallback_globs = fallback_globs;
        if !fallback_globs.contains(&"*.txt".into()) {
            fallback_globs.push("*.txt".into());
        }
        fallback_globs
    };

    let mut editorconfig = EditorConfig::from(input);

    // don't care about preample (usually just "root = true")
    let (_, _preample) = editorconfig.sections.remove(0);

    let mut global_lang_cfg = LangCfg::default();

    let mut hx_editor_cfg = HxEditorCfg::default();
    let mut hx_lang_cfg = BTreeMap::<String, LangCfg>::new();

    // This is used to track which actual language these glob languages
    // belong to, in order to generate textobject queries for them.
    let mut glob_languages = BTreeMap::new();

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

            let (dirname, basename) = lang.rsplit_once('/').unwrap_or(("", &lang));

            // simplify glob: foo/**.ext => foo/**/*.ext
            let (dirname, basename) = if basename.starts_with("**") {
                (format!("{dirname}/**"), basename[1..].to_string())
            } else {
                (dirname.into(), basename.into())
            };

            if basename == "*" {
                // This is a path glob that matches all file types
                // unconditionally. We need to generate a synthetic language
                // definition for every known language.
                for supported_lang in languages {
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
                        lang_cfg.trim_trailing_whitespace = Src::hx(false);
                    }

                    let name = make_synthetic_lang_name("glob", &format!("{lang}-{matched_name}"));
                    let mut raw_toml = supported_lang.raw_toml.clone();
                    raw_toml.remove("injection-regex");
                    raw_toml.insert("grammar", matched_name.clone().into());

                    let file_types = supported_lang
                        .file_types
                        .as_ref()
                        .unwrap()
                        .iter()
                        .map(|ft| match ft {
                            FileType::Extension(s) => format!("{dirname}/*.{s}"),
                            FileType::Glob(s) => format!("{dirname}/{s}"),
                        })
                        .map(FileType::Glob)
                        .collect();

                    lang_cfg.raw_toml = Some(raw_toml);
                    lang_cfg.file_types = Some(file_types);
                    glob_languages.insert(name.clone(), matched_name);
                    hx_lang_cfg.insert(name, lang_cfg);
                }

                // one more synthetic language for the fallback globs
                {
                    let mut lang_cfg = lang_cfg.clone();
                    let name = make_synthetic_lang_name("glob", &format!("{lang}-unknown"));
                    let file_types = fallback_globs.iter().cloned().map(FileType::Glob).collect();
                    lang_cfg.file_types = Some(file_types);
                    hx_lang_cfg.insert(name, lang_cfg);
                }
                continue 'header_lang_loop;
            }

            // We want to recognize "package.json" as json so we can copy its
            // config. However, we don't want to treat it exactly like json,
            // otherwise we'd override general "*.json" configuration.
            let basename_is_more_than_extension =
                basename.rsplit_once('.').is_some_and(|(pre, _)| pre != "*");

            let is_path_glob = !lang_is_stupid_extension_glob && contains_glob_char(&lang)
                || basename_is_more_than_extension;

            let ext = basename
                .rsplit_once('.')
                .map(|(_, ext)| ext)
                .unwrap_or(&basename)
                .to_string();

            for supported_lang in languages {
                if supported_lang
                    .file_types
                    .as_ref()
                    .unwrap()
                    .iter()
                    .any(|ft| match ft {
                        FileType::Extension(s) => s == &ext,
                        FileType::Glob(s) => s == &basename,
                    })
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
                        lang_cfg.trim_trailing_whitespace = Src::hx(false);
                    }

                    if is_path_glob {
                        let name = make_synthetic_lang_name("glob", &lang);
                        let mut raw_toml = supported_lang.raw_toml.clone();
                        raw_toml.remove("injection-regex");
                        raw_toml.insert("grammar", matched_name.clone().into());
                        lang_cfg.raw_toml = Some(raw_toml);
                        lang_cfg.file_types = Some(vec![FileType::Glob(lang)]);
                        glob_languages.insert(name.clone(), matched_name);
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
            lang_cfg.file_types = Some(vec![FileType::Glob(lang)]);

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
        global_lang_cfg.trim_trailing_whitespace.into() == Some(true);
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
                lang_cfg.trim_trailing_whitespace = Src::hx(false);
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
        global_lang_cfg.file_types = Some(fallback_globs.into_iter().map(FileType::Glob).collect());
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

    (
        hx_editor_cfg.to_config_toml(rulers),
        languages_toml,
        glob_languages,
    )
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

                // "" and "ignore" sometimes appear as values. These are
                // non-standard so we ignore them.
                //
                // "" :      ../test_data/pandoc
                // "ignore": ../test_data/django
                //
                if !matches!(value, "" | "ignore") {
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

/// Any EditorConfig property may have its specified value or alternatively
/// the value "unset", which means values otherwise inherited from a previous
/// section should be ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Property<T> {
    Unset,
    Value(T),
}

/// This lets us distinguish where a value came from. This is relevant for the
/// EditorConfig "unset" property. We don't want to unset properties that came
/// from the Helix config, only the ones from the EditorConfig.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Src<T> {
    ec: Option<Property<T>>,
    hx: Option<T>,
}

impl<T> Default for Src<T> {
    fn default() -> Self {
        Self { ec: None, hx: None }
    }
}

impl<T: Copy> Src<T> {
    fn hx(val: T) -> Self {
        Self {
            ec: None,
            hx: Some(val),
        }
    }

    fn is_some(&self) -> bool {
        self.ec.is_some_and(|p| !matches!(p, Property::Unset)) || self.hx.is_some()
    }

    fn is_none(&self) -> bool {
        !self.is_some()
    }

    fn into(self) -> Option<T> {
        match (self.ec, self.hx) {
            (Some(Property::Value(val)), _) => Some(val),
            (_, Some(val)) => Some(val),
            _ => None,
        }
    }
}

impl<T: FromStr> Src<T> {
    fn parse_ec_prop(s: &str) -> Self {
        let ec = match s {
            "unset" => Some(Property::Unset),
            _ => s.parse().ok().map(Property::Value),
        };
        Self { ec, hx: None }
    }
}

#[derive(Debug, Clone, Default)]
pub struct LangCfg {
    size: Src<usize>,
    style: Src<IndentStyle>,
    tab_width: Src<usize>,
    max_line_length: Src<usize>,
    trim_trailing_whitespace: Src<bool>,
    // not part of editorconfig, used to generate custom configs for languages
    // unsupported by Helix
    file_types: Option<Vec<FileType>>,
    // not part of editorconfig, used to generate custom configs for synthetic
    // languages used to support arbitrary path globs
    raw_toml: Option<toml_edit::Table>,
}

impl FromStr for IndentStyle {
    type Err = ();

    fn from_str(style: &str) -> Result<Self, Self::Err> {
        match style.to_lowercase().as_str() {
            "tab" => Ok(Tab),
            "space" => Ok(Space),
            _ => Err(()),
        }
    }
}

impl LangCfg {
    fn with_defaults_from(&mut self, other: &LangCfg) -> &mut Self {
        fn resolve<T: Copy>(it: &mut Src<T>, other: Src<T>) {
            if it.ec.is_none() {
                it.ec = other.ec;
            }
            if it.hx.is_none() {
                it.hx = other.hx;
            }
        }
        resolve(&mut self.size, other.size);
        resolve(&mut self.style, other.style);
        resolve(&mut self.tab_width, other.tab_width);
        resolve(&mut self.max_line_length, other.max_line_length);
        resolve(
            &mut self.trim_trailing_whitespace,
            other.trim_trailing_whitespace,
        );
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
        if self.tab_width.is_some() && self.style.into() != Some(Space) {
            if self.style.into() == Some(Tab) {
                // Do nothing. indent_size has precedence over
                // tab_width if they both come from .editorconfig,
                // but we don't want to override an editorconfig's
                // tab_width with an indent_size from the default
                // languages.toml.
            } else {
                match other_style {
                    Tab => self.style = Src::hx(Tab),
                    Space => {
                        // self.style is none and other style is space.
                        // tab_width will be overruled.
                        self.style = Src::hx(Space);
                        if self.size.is_none() {
                            self.size = Src::hx(other_size);
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
                self.size = Src::hx(other_size)
            }
            if self.style.is_none() {
                self.style = Src::hx(other_style)
            }
        }
        self
    }

    fn from(section: &BTreeMap<Key, &str>) -> Self {
        let size = section
            .get(&Key::IndentSize)
            .map(|s| Src::parse_ec_prop(s))
            .unwrap_or_default();
        let tab_width = section
            .get(&Key::TabWidth)
            .map(|s| Src::parse_ec_prop(s))
            .unwrap_or_default();
        let style = section
            .get(&Key::IndentStyle)
            .map(|s| Src::parse_ec_prop(s))
            .unwrap_or_default();
        let max_line_length = section
            .get(&Key::MaxLineLength)
            .map(|s| Src::parse_ec_prop(s))
            .unwrap_or_default();
        let trim_trailing_whitespace = section
            .get(&Key::TrimTrailingWhitespace)
            .map(|s| Src::parse_ec_prop(s))
            .unwrap_or_default();
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
            let Some(indent_style) = self.style.into() else {
                break 'indent None;
            };
            match (indent_style, self.size.into(), self.tab_width.into()) {
                (Space, Some(size), _) => Some((" ".repeat(size), size)), // tab_width doesn't affect space
                (Tab, Some(size), _) | (Tab, None, Some(size)) => Some(("\t".into(), size)),
                (Space, None, _) | (Tab, None, None) => None,
            }
        };
        if indent.is_none()
            && self.max_line_length.is_none()
            && self.trim_trailing_whitespace.into() != Some(true)
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
                .map(|ft| match ft {
                    FileType::Extension(s) => toml_edit::Value::from(s),
                    FileType::Glob(ft) => {
                        let mut m = toml_edit::InlineTable::new();
                        m.insert("glob", ft.as_str().into());
                        toml_edit::Value::from(m)
                    }
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

        if let Some(max_line_length) = self.max_line_length.into() {
            t.insert("text-width", (max_line_length as i64).into());
            if rulers {
                let len: toml_edit::Value = ((max_line_length + 1) as i64).into();
                let array: toml_edit::Array = [len].into_iter().collect();
                t.insert("rulers", array.into());
            }
        }

        if let Some(true) = self.trim_trailing_whitespace.into() {
            let mut m = toml_edit::InlineTable::new();
            m.insert("command", "ec2hx".into());
            let arg = toml_edit::Value::from("trim-trailing-whitespace");
            let args: toml_edit::Array = [arg].into_iter().collect();
            m.insert("args", args.into());
            t.insert("formatter", m.into());
            t.insert("auto-format", true.into());
        }

        for (_, v) in t.iter_mut() {
            if v.is_table() {
                let inline = v.clone().into_table().unwrap().into_inline_table();
                *v = toml_edit::value(inline);
            }
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
        let (config_toml, languages_toml, _) =
            ec2hx(&languages, &input, vec!["*.foo".into()], false);
        insta::assert_snapshot!("conf", config_toml);
        insta::assert_snapshot!("lang", languages_toml);
    });
}

#[test]
fn rulers() {
    let languages = parse::languages(DEFAULT_LANGUAGES);
    // global rulers
    let input = std::fs::read_to_string("test_data/webpack").unwrap();
    let (config_toml, _, _) = ec2hx(&languages, &input, vec![], true);
    insta::assert_snapshot!("rulers-conf", config_toml);
    // language rulers
    let input = std::fs::read_to_string("test_data/php").unwrap();
    let (_, languages_toml, _) = ec2hx(&languages, &input, vec![], true);
    insta::assert_snapshot!("rulers-lang", languages_toml);
}

#[test]
fn merge_langs() {
    let mut languages = vec![
        HelixLangCfg {
            name: "unchanged".into(),
            indent: Some((2, Space)),
            file_types: Some(vec![FileType::Glob("*.unchanged".into())]),
            has_formatter: false,
            raw_toml: toml_edit::Table::new(),
        },
        HelixLangCfg {
            name: "partial".into(),
            indent: None,
            file_types: Some(vec![FileType::Glob("*.partial".into())]),
            has_formatter: true,
            raw_toml: toml_edit::Table::new(),
        },
    ];
    let user_languages = vec![
        HelixLangCfg {
            name: "partial".into(),
            indent: Some((4, Space)),
            file_types: Some(vec![
                FileType::Glob("*.partial".into()),
                FileType::Glob("*.partial.local".into()),
            ]),
            has_formatter: false,
            raw_toml: toml_edit::Table::new(),
        },
        HelixLangCfg {
            name: "new".into(),
            indent: Some((3, Tab)),
            file_types: Some(vec![FileType::Glob("*.new".into())]),
            has_formatter: false,
            raw_toml: toml_edit::Table::new(),
        },
    ];
    let expected = vec![
        HelixLangCfg {
            name: "unchanged".into(),
            indent: Some((2, Space)),
            file_types: Some(vec![FileType::Glob("*.unchanged".into())]),
            has_formatter: false,
            raw_toml: toml_edit::Table::new(),
        },
        HelixLangCfg {
            name: "partial".into(),
            indent: Some((4, Space)),
            file_types: Some(vec![
                FileType::Glob("*.partial".into()),
                FileType::Glob("*.partial.local".into()),
            ]),
            has_formatter: true,
            raw_toml: toml_edit::Table::new(),
        },
        HelixLangCfg {
            name: "new".into(),
            indent: Some((3, Tab)),
            file_types: Some(vec![FileType::Glob("*.new".into())]),
            has_formatter: false,
            raw_toml: toml_edit::Table::new(),
        },
    ];
    merge_languages(&mut languages, user_languages);

    assert_eq!(languages, expected);
}

#[test]
fn glob_langs() {
    let languages = parse::languages(DEFAULT_LANGUAGES);
    let input = std::fs::read_to_string("test_data/linux").unwrap();
    let (_, _, glob_languages) = ec2hx(&languages, &input, vec![], false);
    insta::assert_snapshot!(format!("{glob_languages:#?}"), @r#"
    {
        "ec2hx-glob-lang-tools-perf-**.py": "python",
        "ec2hx-glob-lang-tools-power-**.py": "python",
        "ec2hx-glob-lang-tools-rcu-**.py": "python",
        "ec2hx-glob-lang-tools-testing-kunit-**.py": "python",
    }
    "#);
}
