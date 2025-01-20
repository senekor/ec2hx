# Changelog

<!-- https://keepachangelog.com/en/1.1.0/ -->

## [Unreleased]

### Changed

### Added

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
