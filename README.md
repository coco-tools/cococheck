# CoCoCheck

[![CI](https://github.com/coco-tools/cococheck/actions/workflows/ci.yml/badge.svg)](https://github.com/coco-tools/cococheck/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

CoCoCheck checks that the commit messages in a Git repository follow the
[Conventional Commits](https://www.conventionalcommits.org/) specification. It is fast and
self-contained, written in Rust and distributed as a single binary with no runtime dependencies.

```text
Commit 3/3 2819a1a "fix(api): Handle errors." failed:

error[no_description_terminal_punctuation]: Description should not end with terminal punctuation '.'.
  |
1 | fix(api): Handle errors.
  |                        ^

CoCoCheck finished checking 3 commits.
1 commit failed the checks.
```

> [!NOTE]
> CoCoCheck is in early development. The command-line interface and the configuration format may
> change in incompatible ways before version 1.0.

## Features

- **A single binary** for Linux, macOS, and Windows, with no runtime dependencies: no Node.js,
  Python, or Rust toolchain is needed.
- **Clear reports** that name the violated rule and mark the offending part of the message.
- **16 configurable rules** for the header, the body, and the footers, each with a severity of
  `error` or `warning`. The defaults work without any configuration.
- **Sensible exceptions:** The messages that Git creates for merge, revert, and fixup commits are
  ignored by default, and further messages or authors, like bots, can be ignored.
- **Checks wherever you commit:** In a `commit-msg` hook, also with pre-commit, Lefthook, or husky,
  and in CI, with the [GitHub Action](https://github.com/coco-tools/cococheck-action) or in any
  other pipeline.
- **Flexible commit selection** with Git revision ranges like `main..`, or all commits since the
  latest tag.
- **JSON output** for processing the results in other tools.

## Installation

Download the binary for your platform from the
[latest release](https://github.com/coco-tools/cococheck/releases/latest), for example on Linux on
x86_64:

```shell
curl -sSfL https://github.com/coco-tools/cococheck/releases/latest/download/cococheck-x86_64-unknown-linux-musl.tar.gz \
  | tar -xz -C ~/.local/bin cococheck
```

Or install it with Cargo: [cargo-binstall](https://github.com/cargo-bins/cargo-binstall) downloads
the prebuilt binary, and `cargo install` builds it from source with Rust 1.89 or later:

```shell
cargo binstall cococheck
cargo install cococheck
```

See [Installation](docs/installation.md) for all platforms and further options.

## Quick start

Check all commits up to `HEAD`, only the commits of the current branch that are not on `main`, or
the commits since the latest tag:

```shell
cococheck
cococheck main..
cococheck --from-latest-tag
```

To check each commit message before the commit is created, create the `commit-msg` hook
`.git/hooks/commit-msg` with the following content, and make it executable. See
[Git hooks](docs/hooks.md) for setting it up with a hook manager.

```shell
#!/bin/sh
exec cococheck --message-file "$1"
```

To check the commits of pull requests and pushes in GitHub Actions, use the
[CoCoCheck action](https://github.com/coco-tools/cococheck-action). See
[Using CoCoCheck in CI](docs/ci.md) for a complete workflow and for other CI systems.

```yaml
    steps:
      - uses: actions/checkout@v7
        with:
          # Fetch the full history, so that the commits to check can be found.
          fetch-depth: 0
      - uses: coco-tools/cococheck-action@v1
```

CoCoCheck exits with status `1` if a commit fails the checks, and with status `2` if an error
occurs. See [Usage](docs/usage.md) for all options, the output formats, and the exit status.

## Configuration

CoCoCheck reads its configuration from a file named `cococheck.yml` or `.cococheck.yml` in the root
of the repository being checked; the `.yaml` extension works as well. Every setting is optional, so
a configuration file only needs to contain what differs from the defaults:

```yaml
rules:
  allowed_scopes:
    scopes: [cli, core]
  description_case:
    case: sentence
  header_length:
    severity: warning
  required_footers:
    tokens: [Signed-off-by]
```

To see the effective configuration, including the defaults of all omitted settings, run
`cococheck --print-config`. See [Configuration](docs/configuration.md) and [Rules](docs/rules.md)
for the full reference.

## Documentation

- [Installation](docs/installation.md): Prebuilt binaries for all platforms, and building from
  source.
- [Usage](docs/usage.md): The command-line options, selecting commits, the output, the JSON format,
  and the exit status.
- [Git hooks](docs/hooks.md): Checking commit messages in a `commit-msg` hook, by hand or with a
  hook manager.
- [Using CoCoCheck in CI](docs/ci.md): Checking pull requests and pushes in GitHub Actions and
  GitLab CI.
- [Configuration](docs/configuration.md): The configuration file, ignored and forbidden commits,
  and the parser.
- [Rules](docs/rules.md): All rules, their settings, and how to change their severity.

## Contributing

Bug reports, feature requests, and pull requests are welcome. See
[`CONTRIBUTING.md`](CONTRIBUTING.md) for how to build and test CoCoCheck, the layout of the
project, and the conventions for commits. Please report security vulnerabilities privately as
described in [`SECURITY.md`](SECURITY.md).

## License

CoCoCheck is licensed under the [Apache License, Version 2.0](LICENSE).
