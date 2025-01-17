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

    let mut buffer = String::from(
        "\
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Language {
    pub(crate) name: &'static str,
    pub(crate) file_types: &'static [&'static str],
}

pub(crate) static LANGUAGES: &[Language] = &[\n",
    );

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
        buffer.push_str("],\n},\n");
    }
    buffer.push_str("];\n");

    std::fs::write(std::env::var("OUT_DIR").unwrap() + "/lang.rs", buffer).unwrap();
}
