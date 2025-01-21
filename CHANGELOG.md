# Changelog

<!-- https://keepachangelog.com/en/1.1.0/ -->

## [Unreleased]

### Breaking

### Changed

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

### Fixed

## [1.1.0] - 2025-01-21

### Changed

- If a Helix configuration already exists, the new one is now written as a
  patch against the old one with instructions on how to apply it.

### Added

- Additional file globs to which global configuration should be applied can now
  be specified on the command line with `--fallback-globs`. Without this option,
  global configuration is only applied to languages Helix knows about.

## [1.0.0] - 2025-01-17

### Added

- First version.

[Unreleased]: https://github.com/senekor/ec2hx/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/senekor/ec2hx/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/senekor/ec2hx/releases/tag/v1.0.0
