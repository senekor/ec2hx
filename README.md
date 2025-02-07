# ec2hx - EditorConfig to Helix

This is a CLI tool that translates an [EditorConfig] file to a project-specific configuration for the [Helix] editor.

## Usage

1. Install using your preferred method:
   | using [binstall](https://github.com/cargo-bins/cargo-binstall) | `cargo binstall ec2hx` |
   | --: | :-- |
   | from source | `cargo install ec2hx` |
   | script, manual etc. | [see release page](https://github.com/senekor/ec2hx/releases/latest) |

2. Run `ec2hx` in your project directory.

3. Start Helix / reload its config.

## Setting expectations

EditorConfig is much more flexible than the config of Helix, which means support is only partial.
However, I believe the support is more than sufficient for "sane" configurations.
This section describes in detail what is and isn't supported.

### Supported keys:

- The keys `end_of_line` and `insert_final_newline` are only supported as a global setting, not per-language.
  That means the keys will only be translated to the Helix config if they appear in the global `[*]` section.
  If they appear in any other section, they will be ignored.
  (These keys are represented in the file `.helix/config.toml`.)
- The keys `indent_style`, `indent_size` and `tab_width` are supported per-language.
  However, they cannot be applied multiple times to the same language with different values.[^1]
  Therefore, sections that contain arbitrary globs are ignored.
  Only sections that look like they cleanly map to a set of file types are considered.
  (These keys are represented in the file `.helix/languages.toml`.)
- All other keys do not map to a config in Helix, so they will be ignored.

### File processing:

The CLI only reads the toplevel `.editorconfig`.
Supporting multiple `.editorconfig` files in a directory hierarchy leads to the same problem as arbitrary globs in section headers.[^1]

### Glob expressions:

Only globs that look like they map cleanly to one or several file types are supported.
The supported glob characters are: `*`, `{}`, `[]`

Examples of supported section headers:
- `[Makefile]`
- `[*.py]`
- `[*.{json,js}]`
- `[*.[ch]]`
- `[{*.{awk,c,dts},Kconfig,Makefile}]` (nested `{}`)

Globs which match against paths (i.e. which contain `/` or `**`) are not supported.[^1]

The following special characters aren't used in any config I'm testing against: `?`, `..`, `!`, `\`.
Sections which contain them in the header will be ignored.
If you think one of these characters should be supported, please open an issue about it and ideally provide an example config that uses it.

### File types:

As mentioned in the "supported keys" section, in Helix, indentation can only be configured for a specific language.
If indentation is configured in the global EditorConfig section (`[*]`), the generated Helix config will have a section for _all_ its supported languages.
However, that's still not perfect!
Languages that don't appear in the default languages.toml of Helix are left out.
The CLI hardcodes some additional ones just for this purpose, for example `.txt`.
You can specify additional file globs to which the global config should apply via the CLI, for example:
```sh
ec2hx --fallback-globs '*.foo,*.bar'
```

(If you have a file extension that appears explicitly in a section header, e.g. `[*.foobar]`, the CLI should already generate an appropriate custom language definition for you.)

## Contributing

A good way to contribute is to provide an example EditorConfig file that you think could be handled better.
You can open an issue about it or a PR adding it to the `test_data/` directory next to the other examples.
It will automatically be picked up by the snapshot tests (using [insta](https://insta.rs/)).

In order to match the EditorConfig section header with a Helix language, the default languages.toml is parsed in a build script.
The URL is pinned to a stable release of Helix and needs to be updated manually.
I think this should be fine, since changes to this file are about rather niche languages at this point.

[^1]: While indentation style can only be configured once per language in Helix, it is technically possible to define arbitrary custom languages.
      That means one could define one pseudo language for every different idententation style in the same project.
      Most language configuration options are copied from the real one, except for the `file-types` key, which is set to the desired glob.
      Out of morbid curiosity, I confirmed that this actually works.
      However, I'm not gonna implement that cursed idea.
      People working on projects with a deranged `.editorconfig` (_cough_ [linux kernel](https://github.com/torvalds/linux/blob/7da9dfdd5a3dbfd3d2450d9c6a3d1d699d625c43/.editorconfig) _cough_) can simply write that godforsaken Helix config by hand.

[EditorConfig]: https://editorconfig.org/
[Helix]: https://helix-editor.com/
