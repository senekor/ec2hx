use std::{fs, process::exit};

fn main() {
    if std::env::args().count() > 1 {
        println!(
            "\
ec2hx - EditorConfig to Helix

Run this in same directory as your .editorconfig file to generate a matching
Helix configuration for it. The Helix config lives in a .helix/ directory that
ignores itself from version control.

Usage:
     no arguments -> do the thing
    any arguments -> print this

For more information, visit <https://github.com/senekor/ec2hx>"
        );
        exit(1);
    }

    let Ok(editorconfig) = std::fs::read_to_string(".editorconfig") else {
        println!("ERROR: Failed to read the .editorconfig file.");
        println!("       Please check your current working directory.");
        exit(1);
    };

    let (config_toml, languages_toml) = ec2hx::ec2hx(&editorconfig);

    fs::create_dir_all(".helix").expect("failed to create .helix directory");
    std::env::set_current_dir(".helix").expect("failed to cd into .helix directory");

    if !fs::exists(".gitignore").is_ok_and(|b| b) {
        fs::write(".gitignore", "*\n").expect("failed to write .helix/.gitignore");
    }
    if fs::exists("languages.toml").is_ok_and(|b| b) {
        println!("WARN: .helix/languages.toml already exists.");
        println!("      Writing to .helix/languages.new.toml instead.");
        println!("      Compare and swap them manually if you like.");
        fs::write("languages.new.toml", languages_toml)
            .expect("failed to write .helix/languages.new.toml");
    } else {
        fs::write("languages.toml", languages_toml).expect("failed to write .helix/languages.toml");
    }
    if !config_toml.is_empty() {
        if fs::exists("config.toml").is_ok_and(|b| b) {
            println!("WARN: .helix/config.toml already exists.");
            println!("      Writing to .helix/config.new.toml instead.");
            println!("      Compare and swap them manually if you like.");
            fs::write("config.new.toml", config_toml)
                .expect("failed to write .helix/config.new.toml");
        } else {
            fs::write("config.toml", config_toml).expect("failed to write .helix/config.toml");
        }
    }
}
