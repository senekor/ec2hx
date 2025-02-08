# Changelog

<!-- https://keepachangelog.com/en/1.1.0/ -->

## Unreleased

[compare changes](https://github.com/senekor/ec2hx/compare/v1.3.0...HEAD)

### Breaking

### Changed

### Added

- `ec2hx` now attempts to fetch and cache the latest version of Helix' default
  languages.toml file. While a pinned version is still embedded as fallback,
  this will reduce the need to publish updated versions of `ec2hx` as more
  languages are added upstream. The cache is refreshed up to once per week.

### Fixed

- If `max_line_length` is only set in the global `[*]` editorconfig section,
  `ec2hx` will now generate `text-width` overrides for every language in
  addition to the global setting.

## 1.3.0 - 2025-02-07

[compare changes](https://github.com/senekor/ec2hx/compare/v1.2.0...v1.3.0)

### Added

- The `max_line_length` key is now supported. From the Helix documentation:
  > Used for the `:reflow` command and soft-wrapping if `soft-wrap.wrap-at-text-width` is set

- A new CLI flag `--rulers` adds rulers matching `max_line_length`.

## 1.2.0 - 2025-02-07

[compare changes](https://github.com/senekor/ec2hx/compare/v1.1.0...v1.2.0)

### Added

- If the EditorConfig file only specifies one of `indent_style` and `indent_size`
  globally or for a specific language, the values in the default `languages.toml`
  of Helix will be used to fill in the gaps and generate a complete configuration
  for more languages. For example, consider the following EditorConfig:
  ```editorconfig
  [Makefile]
  indent_size = 8
  ```
  This will now result in the following Helix configuration, because the default
  `languages.toml` already specifies that Makefile should be indented with tabs.
  ```toml
  indent = { tab-width = 8, unit = "\t" }
  ```

- Square brackets in section header globs are now supported. For example,
  C-files can be configured with the header `[*.[ch]]`.

- The `tab_width` key is now supported.

### Fixed

- Properly ignore sections that contain `\` in the header.

## 1.1.0 - 2025-01-21

[compare changes](https://github.com/senekor/ec2hx/compare/v1.0.0...v1.1.0)

### Changed

- If a Helix configuration already exists, the new one is now written as a
  patch against the old one with instructions on how to apply it.

### Added

- Additional file globs to which global configuration should be applied can now
  be specified on the command line with `--fallback-globs`. Without this option,
  global configuration is only applied to languages Helix knows about.

## 1.0.0 - 2025-01-17

[compare changes](https://github.com/senekor/ec2hx/tree/v1.0.0)

### Added

- First version.
