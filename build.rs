use std::fmt::Write;

const DEFAULT_LANGUAGES_TOML_URL: &str =
    "https://raw.githubusercontent.com/helix-editor/helix/refs/tags/25.01/languages.toml";

fn main() {
    // don't @ me if you don't have curl, I literally don't care
    let output = std::process::Command::new("curl")
        .arg(DEFAULT_LANGUAGES_TOML_URL)
        .output()
        .unwrap();

    let langauges = String::from_utf8(output.stdout).unwrap();
    let langauges = toml::from_str::<toml::Table>(&langauges).unwrap();
    let languages = &langauges["language"].as_array().unwrap();

    let mut buffer = String::from("&[");

    for lang in languages.iter() {
        let lang = lang.as_table().unwrap();
        buffer.push_str("Language {\n");

        let name = lang.get("name").unwrap().as_str().unwrap();
        writeln!(buffer, "name: {name:?},",).unwrap();

        buffer.push_str("file_types: &[");
        for file_type in lang.get("file-types").unwrap().as_array().unwrap().iter() {
            if let Some(file_type) = file_type.as_str() {
                write!(buffer, "{:?},", file_type).unwrap();
            } else {
                let file_type = file_type.as_table().unwrap().get("glob").unwrap();
                write!(buffer, "{:?},", file_type.as_str().unwrap()).unwrap();
            }
        }
        buffer.push_str("],\n");

        let indent = if let Some(indent) = lang.get("indent") {
            let size = indent.get("tab-width").unwrap().as_integer().unwrap();
            let unit = indent.get("unit").unwrap().as_str().unwrap();
            let style = if unit.starts_with(' ') {
                "Space"
            } else {
                // This is exactly how Helix behaves, everything that's not a
                // space is a tab.
                "Tab"
            };
            format!("size: Some({size}), style: Some({style}), tab_width: None")
        } else {
            "size: None, style: None, tab_width: None".into()
        };

        writeln!(
            buffer,
            "cfg: LangCfg {{ {indent}, max_line_length: None, file_types: vec![] }},"
        )
        .unwrap();

        buffer.push_str("},\n");
    }
    buffer.push(']');

    std::fs::write(std::env::var("OUT_DIR").unwrap() + "/lang.rs", buffer).unwrap();
}
