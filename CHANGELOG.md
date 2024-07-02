<!-- Instructions

This changelog follows the patterns described here: <https://keepachangelog.com/en/>.

Subheadings to categorize changes are `added, changed, deprecated, removed, fixed, security`.

-->

# Changelog

The latest published Parley release is [0.1.0](#010---2024-05-01) which was released on 2024-05-01.
You can find its changes [documented below](#010---2024-05-01).

## [Unreleased]

### Added

### Parley

- Example using tiny-skia which renders into a png ([#55] by [@nicoburns])
    - Breaking change: There is now a blanket implementation for `Brush`.
- A swash example which renders into a png ([#54] by [@nicoburns])

### Changed

### General

- Repository layout updated to match Linebender standard ([#59] by [@waywardmonkeys])

### Parley

- Emoji clusters now get an Emoji family added by default ([#56] by [@dfrg])

### Fontique

- Removed unsafe code from fontconfig cache ([#78] by [@waywardmonkeys])

### Fixed

- Search correct paths for fonts on Apple platforms ([#71] by [@waywardmonkeys])

## Removed

- Breaking change: removed conversion to/from `icu_properties::Script` for `fontique::Script` ([#72] by [@waywardmonkeys])
    - This can be restored by using the `icu_properties` feature of `fontique`.

## [0.1.0] - 2024-05-01

This release has an [MSRV] of 1.70.

- Initial release

[MSRV]: README.md#minimum-supported-rust-version-msrv

[@nicoburns]: https://github.com/nicoburns
[@dfrg]: https://github.com/dfrg
[@waywardmonkeys]: https://github.com/waywardmonkeys

[#54]: https://github.com/linebender/parley/pull/54
[#55]: https://github.com/linebender/parley/pull/55
[#56]: https://github.com/linebender/parley/pull/56
[#59]: https://github.com/linebender/parley/pull/59
[#78]: https://github.com/linebender/parley/pull/78
[#71]: https://github.com/linebender/parley/pull/71
[#72]: https://github.com/linebender/parley/pull/72

[Unreleased]: https://github.com/linebender/parley/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/linebender/parley/releases/tag/v0.1.0
