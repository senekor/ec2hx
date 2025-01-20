use std::{fs, process::exit};

use clap::Parser;

/// ec2hx - convert EditorConfig to Helix configuration
///
/// Simply run ec2hx in a directory with a .editorconfig file and a .helix
/// directory will be generated for you. It contains configuration to match
/// EditorConfig as closely as possible.
///
/// Due to limitations in the configuration of Helix, not all EditorConfig
/// features are supported, but the important ones should work fine
/// (indentation, line ending, final newline).
///
/// The .helix directory will ignore itself using a .helix/.gitignore file, so
/// don't worry about accidentally committing these files to version control.
/// Existing files won't be clobbered, to preserve any manual adjustments you
/// have made.
///
/// For more information, visit <https://github.com/senekor/ec2hx>
#[derive(Debug, clap::Parser)]
#[command(version, about, long_about)]
struct CliArgs {
    /// additional file types to configure
    #[arg(long, value_delimiter=',', long_help = FALLBACK_GLOBS_HELP)]
    fallback_globs: Vec<String>,
}

const FALLBACK_GLOBS_HELP: &str = "\
additional file types to configure

Helix applies some configuration only to specfic languages, not globally.
(e.g. indentation) That means those settings in a global [*] section of a
.editorconfig file won't apply to file types Helix doesn't know about. You can
tell ec2hx to generate a virtual language definition for additional file types,
such that these global configuration options apply to them as well. By default,
.txt files are already treated this way.

Provide a comma-separated list and don't forget to quote the string to prevent
the globs from being interpreted by the shell. For convenience, *.txt is already
included.

Example: --fallback-globs '*.foo,*.bar'";

fn main() {
    let args = CliArgs::parse();

    let Ok(editorconfig) = std::fs::read_to_string(".editorconfig") else {
        println!("ERROR: Failed to read the .editorconfig file.");
        println!("       Please check your current working directory.");
        exit(1);
    };

    let (config_toml, languages_toml) = ec2hx::ec2hx(&editorconfig, args.fallback_globs);

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
