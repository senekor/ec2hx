# ec2hx - EditorConfig to Helix

This is a CLI tool that translates an [EditorConfig] file to a project-specific configuration for the [Helix] editor.

EditorConfig is much more flexible than the config of Helix, which means support is only partial.
Here is a list of notable limitations:

- Supported keys:

  - The keys `end_of_line` and `insert_final_newline` are only supported as a global setting, not per-language.
    That means the keys will only be translated to the Helix config if they appear in the global (`[*]`) section.
    If they appear in any other section, they will be ignored.
  - The keys `indent_style` and `indent_size` are supported per-language.
    This is probably the most important feature and it works.
    However, they cannot be applied multiple times to the same language with different values.[^1]
    Therefore, sections that contain arbitrary globs are ignored.
    Only sections that look like they cleanly map to a set of file types are considered.
  - All other keys do not map to a config in Helix, so they will be ignored.

- File processing:

  The CLI only reads the toplevel `.editorconfig`.
  Supporting multiple `.editorconfig` files in a directory hierarchy leads to the same problem as arbitrary globs in section headers.[^1]

- Glob expressions:

  Only globs that look like they map cleanly to one or several file types are supported, including for example:
  - `[Makefile]`
  - `[*.py]`
  - `[*.{json,js}]`
  - and even `[{*.{awk,c,dts,dtsi,dtso,h,mk,s,S},Kconfig,Makefile,Makefile.*}]`

  Globs that don't map to a set of file types are not supported.[^1]
  For simplicity, any section that includes the following special characters is ignored: `/`, `**`, `?`, `..`, `[`, `]`, `!`, `\`.
  If you think some application of these special characters can and should be supported, feel free to open an issue about it.

The generated Helix config lives in `.helix/config.toml` and `.helix/languages.toml`.
The CLI also generates a gitignore file `.helix/.gitignore` with the content `*`.
This ensures you don't accidentally commit these files into version control.

## Contributing

An easy way to contribute is to provide an example EditorConfig file that you think could be handled better.
You can open an issue about it or even a PR adding it to the `test_data/` directory next to the other examples.
It will automatically be picked up by the snapshot tests (using [insta](https://insta.rs/)).

TODO:

- extract languages from section header

- parse default languages.toml (from helix as submodule?) (at built time?) to match section headers against supported languages and their file-types and globs

- CLI to generate files (config.toml, languages.toml, .gitignore)

- apply global settings to a custom language for generic files, e.g. `*.txt`
  (Helix doesn't have a builtin config for those.)

- add more relevant examples for snapshot testing and analyse the output (linux kernel!)

- release: crate metadata (v1.0), dist and binstall setup, publish to crates.io

- `indent_style` and `indent_size` are currently expected to both be present.
  If only one of them is present, it's ignored.
  (Helix _requires_ both `unit` and `tab-width` to be explicitly configured at the same time.)
  This could be handled better in some cases.
  See for example the [cockroach editorconfig](./test_data/cockroach):
  - Globally, only `indent_size` is set.
    We may be able to combine that with the unit by parsing the default languages.toml.

- The key `trim_trailing_whitespace` could become supported by helix in the future.
  See for exmaple [this PR](https://github.com/helix-editor/helix/pull/8366) for progress.
  An alternative would be to bundle a simple formatter with the CLI.
  Then parse the default `languages.toml` to determine which languages don't already have a formatter.
  (wouldn't want to override a more powerful, language-specific formatter that probably already trims whitespace)
  Add this generic formatter to the language config where appropriate.

[^1]: While indentation style can only be configured once per language in Helix, it is technically possible to define arbitrary custom languages.
      That means one could define one pseudo language for every different idententation style in the same project.
      Most language configuration options are copied from the real one, except for the `file-types` key, which is set to the desired glob.
      Out of morbid curiosity, I confirmed that this actually works.
      However, I'm not gonna implement that cursed idea.
      People working on projects with a deranged `.editorconfig` (_cough_ [linux kernel](https://github.com/torvalds/linux/blob/7da9dfdd5a3dbfd3d2450d9c6a3d1d699d625c43/.editorconfig) _cough_) can simply write that godforsaken Helix config by hand.

[EditorConfig]: https://editorconfig.org/
[Helix]: https://helix-editor.com/
