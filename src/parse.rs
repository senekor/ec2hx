use std::str::FromStr;

use crate::{HelixLangCfg, IndentStyle};

pub fn languages(input: &str) -> Vec<HelixLangCfg> {
    let input = toml_edit::DocumentMut::from_str(input).unwrap();
    input["language"]
        .as_array_of_tables()
        .unwrap()
        .iter()
        .map(|raw_toml| {
            let lang = raw_toml;

            let name = lang.get("name").unwrap().as_str().unwrap().to_string();

            let file_types = lang.get("file-types").map(|ft| {
                ft.as_array()
                    .unwrap()
                    .iter()
                    .map(|file_type| {
                        if let Some(file_type) = file_type.as_str() {
                            file_type.to_string()
                        } else {
                            let file_type =
                                file_type.as_inline_table().unwrap().get("glob").unwrap();
                            file_type.as_str().unwrap().to_string()
                        }
                    })
                    .collect()
            });

            let indent = if let Some(indent) = lang.get("indent") {
                let size = indent
                    .get("tab-width")
                    .unwrap()
                    .as_integer()
                    .unwrap()
                    .try_into()
                    .unwrap();
                let unit = indent.get("unit").unwrap().as_str().unwrap();
                let style = if unit.starts_with(' ') {
                    IndentStyle::Space
                } else {
                    // This is exactly how Helix behaves, everything that's not a
                    // space is a tab.
                    IndentStyle::Tab
                };
                Some((size, style))
            } else {
                None
            };

            let has_formatter =
                lang.get("language-servers").is_some() || lang.get("formatter").is_some();

            HelixLangCfg {
                name,
                indent,
                file_types,
                has_formatter,
                raw_toml: lang.clone(),
            }
        })
        .collect()
}
