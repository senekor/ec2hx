# ec2hx - EditorConfig to Helix

This is a CLI tool that translates an [EditorConfig] file to a project-specific configuration for the [Helix] editor.

## Usage

1. Install using your preferred method:
   | using [binstall](https://github.com/cargo-bins/cargo-binstall) | `cargo binstall ec2hx` |
   | --: | :-- |
   | from source | `cargo install ec2hx` |
   | script, manual etc. | [see release page](https://github.com/senekor/ec2hx/releases/latest) |

2. Run `ec2hx` in your project directory.

3. Start Helix or run `:config-reload`.

## Upstream support

At the time of writing, Helix does not support EditorConfig.
The maintainers don't want to add it in core, they want a plugin to handle it.
However, the plugin system is still a work in progress.
That's why `ec2hx` may be your best option at the moment.

Subscribe to the following issues and PRs to stay up-to-date with developments upstream:
- [issue: Support for EditorConfig](https://github.com/helix-editor/helix/issues/279)
- [PR: Implement EditorConfig support](https://github.com/helix-editor/helix/pull/1777)
- [PR: :trim to remove trailing whitespace](https://github.com/helix-editor/helix/pull/8366)
- [issue: tracking issue: Switch to a scheme based config](https://github.com/helix-editor/helix/issues/10389)
- [PR: Add Steel as an optional plugin system](https://github.com/helix-editor/helix/pull/8675)

## Setting expectations

EditorConfig is more flexible than the config of Helix, which means support is only partial.
However, I believe the support is more than sufficient for "sane" configurations and even a couple insane ones.
This section describes in detail what is and isn't supported.

### Properties

Almost all properties are supported, but there are some caveats.
The only unsupported key is `charset`, which doesn't have a matching Helix configuration key.
For reference, here is the [list of official EditorConfig properties](https://github.com/editorconfig/editorconfig/wiki/EditorConfig-Properties).

- `indent_style` (fully supported)

- `indent_size` (fully supported)

- `tab_width` (overruled by `indent_size`, weird setups where the two don't match are not supported)

- `max_line_length` (use the CLI flag `--rulers` to add matching rulers)

- `end_of_line` (only in the global `[*]` section, not per-language)

- `insert_final_newline` (only in the global `[*]` section, not per-language)

- `trim_trailing_whitespace` has some support.

  It is achieved with a built-in formatter that does the trimming.
  `ec2hx` configures Helix to call `ec2hx trim-trailing-whitespace` to format files.

  However, `ec2hx` doesn't override any formatters or LSPs from the default or user Helix configuration, because they may already handle formatting.
  Unfortunately, you may not even have these installed, or some LSP may be installed but it doesn't support formatting.
  So, there are many false negatives where `ec2hx` won't automatically generate the formatter config for a language even if it might be safe.

  You can fix this by manually adding the below formatter config to either your user configuration (`~/.config/helix/languages.toml`) or the project-specific configuration (`$proj_dir/.helix/languages.toml`).
  The latter is what `ec2hx` generates, but don't worry, it won't write over your manual changes if you run it twice.

  ```toml
  formatter = { command = "ec2hx", args = ["trim-trailing-whitespace"] }
  auto-format = true
  ```

### File processing:

The CLI only reads the toplevel `.editorconfig`.
Adding support for `.editorconfig` files in parent or subdirectories is technically feasible, but I'm not aware of this feature being used in the wild.
Please [open an issue] if you would like this to be supported.

### Glob expressions:

Glob expressions are generally supported.
However, the best support is for section headers that cleanly map to a set of file types.
Examples of such well-supported section headers are:
- `[Makefile]`
- `[*.py]`
- `[*.{json,js}]`
- `[*.[ch]]`
- `[{*.{awk,c,dts},Kconfig,Makefile}]`

Globs which match against paths (e.g. when they contain `/` or `**`) are also supported, but there some caveats.

<details>
<summary>click here if you're interested in how that even works in the first place</summary>

Helix does not directly support configuring properties based on file globs.
It's only possible to set these properties either globally for for a specific language.

The trick is that you can define a custom language which matches against a weirdly specific glob with its file-types key.

So, what `ec2hx` does is first try to figure out the actual file type the glob matches against.
Then it will copy the existing Helix configuration for that language (even respecting your user configuration) to a new artificial language definition.

Helix will then recognize files that match this glob as the synthetic language and apply the appropriate config.

One slightly unfortunate downside of this approach is that syntax highlighting only works for languages that have appropriate queries in the Helix runtime directory.
At the time of writing, Helix doesn't support project specific runtime files.
Therefore, `ec2hx` will generate the necessary queries into your user configuration directory.
For example, that would be `~/.config/helix/runtime/queries` on Linux.
The directories generated by `ec2hx` are prefixed with `ec2hx-glob-lang-`, so there shouldn't be any conflicts.

If you don't like it when programs vomit auto-generated garbage into your config directory...
I agree with you and I'm sorry!
[If Helix adds support for it](https://github.com/helix-editor/helix/issues/12821), it might be possible to avoid this in the future.

</details>

#### No inheritance between path glob sections

Path glob sections don't inhert configuration from previous path glob sections that also match the same files.
They only inherit configuration from previous sections that cleanly map to a file type.
Consider the following example:

```ini
[*.md]
indent_size = 4
indent_style = tab

[docs/**.md]
indent_size = 2

[docs/internal/**.md]
indent_style = space
```

According to the EditorConfig specification, the `[docs/internal/**.md]` section should inherit `indent_size = 2` from the `[docs/**.md]` section, but it only inherits `indent_size = 4` from the `[*.md]` section.

#### Rare special characters are unsupported

The characters `!`, `..` and `\` aren't used in any configuration I'm testing against.
Sections which contain them in the header will be ignored.
I don't think it's possible to support `!`, but `..` and `\` are technically feasible.
Please [open an issue] if you would like them to be supported.

### File types:

Some of the properties like `indent_size` and `indent_style` can only be configured per-language in Helix.
So what happens if they appear in a section that applies to all file types, not just specific ones?
(examples for such sections: `[*]`, `src/**`, `[docs/**]`)

`ec2hx` will generate a language configuration with these properties for _every single language Helix supports_.
Unfortunately, that means this configuration won't apply to file types Helix doesn't recognize.
`*.txt` is a hardcoded exception and you can specify additional file globs to which such config should apply via the CLI, for example:
```sh
ec2hx --fallback-globs '*.foo,*.bar'
```

## Contributing

A good way to contribute is to provide an example EditorConfig file that you think could be handled better.
You can [open an issue] about it or a PR adding it to the `test_data/` directory next to the other examples.
It will automatically be picked up by the snapshot tests (using [insta](https://insta.rs/)).

[EditorConfig]: https://editorconfig.org/
[Helix]: https://helix-editor.com/
[open an issue]: https://github.com/senekor/ec2hx/issues/new
