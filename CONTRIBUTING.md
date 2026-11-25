# Contributing

Bug reports, feature requests, and pull requests are welcome. Please
[open an issue](https://github.com/coco-tools/cococheck/issues) to discuss larger changes before
starting work on them.

Please report security vulnerabilities privately as described in [`SECURITY.md`](SECURITY.md).

## Building and testing

To build the project and run the tests:

```shell
cargo build
cargo test
```

The CI pipeline runs the following checks on every pull request. Run them locally before
submitting changes:

```shell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

The tests run on Linux, macOS, and Windows, and on Linux also with the minimum supported Rust
version, 1.89. If a change requires a newer Rust version, update `rust-version` in
[`Cargo.toml`](Cargo.toml) and the MSRV job in
[`.github/workflows/ci.yml`](.github/workflows/ci.yml) together.

## Commit messages

Commit messages in this repository follow the Conventional Commits specification. The CI pipeline
checks the commits of every pull request, and the commits pushed to `main`, with the
[CoCoCheck action](https://github.com/coco-tools/cococheck-action), which uses a released version of
CoCoCheck. A change to CoCoCheck only affects this check once it is released and adopted by a new
release of the action. To run the check locally with the version of your branch:

```shell
cargo run -p cococheck -- main..
```

## Project layout

The repository is a Cargo workspace with two crates:

- The root directory contains the `cococheck` package with the command-line tool. It is in the
  root, so that [pre-commit](README.md#hook-managers) can install it from the repository.
- [`cococheck-core`](cococheck-core) contains the commit message parser, the configuration, and the
  rules.

The [`docs`](docs) directory contains the reference of the configuration and the rules, and the
description of the [release process](docs/releasing.md) for maintainers.

## Developer Certificate of Origin

By contributing to this project, you certify that you have the right to submit your contribution
under the project's license, as described in the
[Developer Certificate of Origin](https://developercertificate.org/) (DCO). To certify it, add a
`Signed-off-by` footer with your name and email address to every commit, which Git does when you
commit with `git commit -s`:

```text
Signed-off-by: Jane Doe <jane.doe@example.org>
```

The CI pipeline rejects commits without this footer.
