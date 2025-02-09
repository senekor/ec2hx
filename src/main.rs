use std::{
    fs,
    io::{stdin, Read},
    path::Path,
    process::exit,
    time::Duration,
};

use clap::Parser;
use etcetera::base_strategy::{choose_base_strategy, BaseStrategy};

#[derive(Debug, clap::Parser)]
#[command(version, about, long_about = LONG_ABOUT)]
struct CliArgs {
    /// additional file types to configure
    #[arg(long, value_delimiter=',', long_help = FALLBACK_GLOBS_HELP)]
    fallback_globs: Vec<String>,
    /// add rulers matching max_line_length
    #[arg(long)]
    rulers: bool,
    #[command(subcommand)]
    cmd: Option<Subcommand>,
}

#[derive(Debug, clap::Subcommand)]
enum Subcommand {
    /// used internally to apply trim_trailing_withspace via a formatter
    #[command(hide = true)]
    TrimTrailingWhitespace,
}

const LONG_ABOUT: &str = "\
ec2hx - convert EditorConfig to Helix configuration
Simply run ec2hx in a directory with a .editorconfig file and a .helix
directory will be generated for you. It contains configuration to match
EditorConfig as closely as possible.
Due to limitations in the configuration of Helix, not all EditorConfig
features are supported, but the important ones should work fine
(indentation, line ending, final newline).
The .helix directory will ignore itself using a .helix/.gitignore file, so
don't worry about accidentally committing these files to version control.
Existing files won't be clobbered, to preserve any manual adjustments you
have made.
For more information, visit <https://github.com/senekor/ec2hx>";

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

    if let Some(Subcommand::TrimTrailingWhitespace) = args.cmd {
        let mut input = String::new();
        if let Err(err) = stdin().read_to_string(&mut input) {
            eprintln!("failed to read from stdin: {err}");
            exit(1);
        };
        let output = ec2hx::fmt::trim_trailing_whitespace(&input);
        print!("{output}");
        exit(0);
    }

    let Ok(editorconfig) = std::fs::read_to_string(".editorconfig") else {
        println!("ERROR: Failed to read the .editorconfig file.");
        println!("       Please check your current working directory.");
        exit(1);
    };

    let mut languages = match fetch_and_cache_languages() {
        Some(l) => ec2hx::parse::languages(&l),
        None => ec2hx::parse::languages(ec2hx::DEFAULT_LANGUAGES),
    };
    if let Some(user_languages) = read_user_languages() {
        ec2hx::merge_languages(&mut languages, user_languages);
    }

    let (config_toml, languages_toml, glob_languages) =
        ec2hx::ec2hx(&languages, &editorconfig, args.fallback_globs, args.rulers);

    fs::create_dir_all(".helix").expect("failed to create .helix directory");

    if !fs::exists(".helix/.gitignore").is_ok_and(|b| b) {
        fs::write(".helix/.gitignore", "*\n").expect("failed to write .helix/.gitignore");
    }
    try_write(".helix/languages.toml", languages_toml);
    try_write(".helix/config.toml", config_toml);

    if !glob_languages.is_empty() {
        let queries_dir = helix_config_dir().join("runtime").join("queries");
        for (synthetic, actual) in glob_languages {
            let lang_dir = queries_dir.join(synthetic);
            if fs::create_dir_all(&lang_dir).is_err() {
                continue;
            }
            let inherits = format!("; inherits: {actual}");
            let queries = [
                "highlights.scm",
                "injections.scm",
                "locals.scm",
                "indents.scm",
                "textobjects.scm",
            ];
            for query in queries {
                let _ = fs::write(lang_dir.join(query), &inherits);
            }
        }
    }
}

fn fetch_and_cache_languages() -> Option<String> {
    let cache_path = directories::ProjectDirs::from("dev", "buenzli", "ec2hx")?
        .cache_dir()
        .join("languages.toml");

    let stale_cache = match read_cache(&cache_path) {
        Some(CacheContent::Fresh(content)) => return Some(content),
        Some(CacheContent::Stale(content)) => Some(content),
        None => None,
    };

    let Some(fetched_languages) = fetch_languages() else {
        return stale_cache;
    };

    // try to update cache, ignore failure
    let _ = std::fs::create_dir_all(cache_path.parent().unwrap());
    let _ = std::fs::write(cache_path, &fetched_languages);

    Some(fetched_languages)
}

enum CacheContent {
    Fresh(String),
    Stale(String),
}

fn read_cache(cache_path: &Path) -> Option<CacheContent> {
    let content = std::fs::read_to_string(cache_path).ok()?;

    let cache_metadata = std::fs::metadata(cache_path).ok()?;
    let mtime = cache_metadata.modified().ok()?;
    let one_week = Duration::from_secs(60 * 60 * 24 * 7);

    if mtime.elapsed().ok()? < one_week {
        Some(CacheContent::Fresh(content))
    } else {
        Some(CacheContent::Stale(content))
    }
}

fn fetch_languages() -> Option<String> {
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(1))
        .build()
        .ok()?
        .get(
            "https://raw.githubusercontent.com/helix-editor/helix/refs/heads/master/languages.toml",
        )
        .send()
        .ok()?
        .text()
        .ok()
}

/// copied from helix-loader/src/lib.rs to match Helix' behavior
fn helix_config_dir() -> std::path::PathBuf {
    let strategy = choose_base_strategy().expect("Unable to find the config directory!");
    let mut path = strategy.config_dir();
    path.push("helix");
    path
}

fn read_user_languages() -> Option<Vec<ec2hx::HelixLangCfg>> {
    let path = helix_config_dir().join("languages.toml");
    let content = std::fs::read_to_string(&path).ok()?;
    Some(ec2hx::parse::languages(&content))
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
