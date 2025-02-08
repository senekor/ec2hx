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
    /// add rulers matching max_line_length
    #[arg(long)]
    rulers: bool,
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

    let languages = ec2hx::parse::languages(ec2hx::DEFAULT_LANGUAGES);

    let (config_toml, languages_toml) =
        ec2hx::ec2hx(&languages, &editorconfig, args.fallback_globs, args.rulers);

    fs::create_dir_all(".helix").expect("failed to create .helix directory");

    if !fs::exists(".helix/.gitignore").is_ok_and(|b| b) {
        fs::write(".helix/.gitignore", "*\n").expect("failed to write .helix/.gitignore");
    }
    try_write(".helix/languages.toml", languages_toml);
    try_write(".helix/config.toml", config_toml);
}

fn try_write(name: &str, contents: String) {
    if contents.is_empty() {
        return;
    }
    if let Ok(prev_contents) = fs::read_to_string(name) {
        if prev_contents == contents {
            return;
        }
        let name_new = &format!("{name}.new");
        let name_patch = &format!("{name}.patch");

        println!("WARN: {name} already exists.");
        if fs::write(name_new, &contents).is_err() {
            panic!("failed to write {name_new}");
        }

        // Attempt to produce a diff against the existing file. This makes it
        // easier for users to assess and apply the changes.
        let create_diff = || -> Option<()> {
            fs::write(name_new, &contents).ok()?;

            let diff_output = std::process::Command::new("diff")
                .arg("--unified")
                .arg(name)
                .arg(name_new)
                .output()
                .ok()?;
            fs::write(name_patch, diff_output.stdout).ok()?;

            // don't care if *.new file wasn't cleaned up
            let _ = fs::remove_file(name_new);
            Some(())
        };

        if create_diff().is_some() {
            println!("      Writing the diff to {name_patch} instead.");
            println!("      Run the following command to apply the patch:");
            println!();
            println!("      patch {name} < {name_patch}");
            println!();
        } else {
            println!("      Writing to {name_new} instead.");
            println!("      Compare and swap them manually if you like.");
        }
    } else if fs::write(name, contents).is_err() {
        panic!("failed to write {name}");
    }
}
