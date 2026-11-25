# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0](https://github.com/coco-tools/cococheck/compare/v0.1.0...v0.2.0) - 2026-11-25

### Added

- [**breaking**] Exit with status 2 if an error occurs
- Show the position of failed commits in the checked range
- Show the short ID and title of failed commits

### Fixed

- *(core)* Reject unknown options of rules
- Name the value of the `--config` option `PATH` in the help

### Updated dependencies

- Update Rust crate regex to 1.13.2
- Update Rust crate serde-saphyr to 2.2.0
- Update Rust crate clap to 4.6.8
- Update Rust crate serde-saphyr to 2.1.1
- Update Rust crate log to 0.4.35
- Lock file maintenance
- Update Rust crate annotate-snippets to 0.12.18
- Pin dependencies

## [0.1.0](https://github.com/coco-tools/cococheck/releases/tag/v0.1.0) - 2026-11-22

Initial release of CoCoCheck, a fast checker for
[Conventional Commits](https://www.conventionalcommits.org/) messages that is distributed as a
single binary.

See the [README](https://github.com/coco-tools/cococheck/blob/v0.1.0/README.md) for the installation
and the usage, and the reference of the
[configuration](https://github.com/coco-tools/cococheck/blob/v0.1.0/docs/configuration.md) and the
[rules](https://github.com/coco-tools/cococheck/blob/v0.1.0/docs/rules.md).
