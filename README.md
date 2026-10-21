# CoCoCheck

[![CI](https://github.com/mnonnenmacher/cococheck/actions/workflows/ci.yml/badge.svg)](https://github.com/mnonnenmacher/cococheck/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

CoCoCheck checks that the commit messages in a Git repository follow the
[Conventional Commits](https://www.conventionalcommits.org/) specification. It is a fast,
self-contained alternative to [commitlint](https://commitlint.js.org/), written in Rust and
distributed as a single binary with no runtime dependencies.

> [!NOTE]
> CoCoCheck is in early development. The command-line interface and the configuration format may
> change in incompatible ways before version 1.0.

## Contents

- [Installation](#installation)
- [Usage](#usage)
- [Configuration](#configuration)
- [Project layout](#project-layout)
- [Contributing](#contributing)
- [License](#license)

## Installation

CoCoCheck requires Rust 1.89 or later. Install the latest version from the Git repository with
Cargo:

```shell
cargo install --git https://github.com/mnonnenmacher/cococheck cococheck-cli
```

This installs the `cococheck` binary into Cargo's binary directory, usually `~/.cargo/bin`.

To build from source instead, clone the repository and build the release binary:

```shell
git clone https://github.com/mnonnenmacher/cococheck.git
cd cococheck
cargo build --release
```

The binary is written to `target/release/cococheck`.

## Usage

Run `cococheck` in a Git repository to check all commits from the first commit up to `HEAD`:

```shell
cococheck
```

CoCoCheck parses each commit message, applies the enabled [rules](#rules), and reports every
commit that fails:

```text
Commit 00e0da671f8fe523dc47c78033e776f803798424 failed to parse:

error: invalid header: expected ': ' after the type
  |
1 | Update stuff.
  |       ^

Commit 2819a1a133fa4e1fcefb08f90c1bb2bb8f675c93 failed:

error[NoDescriptionTerminalPunctuation]: Description should not end with terminal punctuation (., !, ?).
  |
1 | fix(api): Handle errors.
  |                        ^

CoCoCheck finished checking 3 commits.
2 commits failed the checks.
```

### Options

| Option                  | Description                                          | Default         |
|-------------------------|------------------------------------------------------|-----------------|
| `-i`, `--input <INPUT>` | Path to the Git repository to check.                 | `.`             |
| `-c`, `--config <PATH>` | Path to the [configuration](#configuration) file.    | See below       |
| `--from <FROM>`         | First commit to check. The commit itself is checked. | The root commit |
| `--to <TO>`             | Last commit to check. The commit itself is checked.  | `HEAD`          |
| `--info`                | Log informational messages, such as passed commits.  |                 |
| `--debug`               | Log debug messages, including the parsed commits.    |                 |
| `-h`, `--help`          | Print help.                                          |                 |
| `-V`, `--version`       | Print the version.                                   |                 |

Without `--config`, CoCoCheck reads `cococheck.yml` from the repository given by `--input`, and
uses the default configuration if that file does not exist. A file given with `--config` must exist;
otherwise CoCoCheck fails with an error.

`--from` and `--to` accept any Git revision that resolves to a commit, such as a commit ID, a branch
or tag name, or an expression like `HEAD~3`. The range is inclusive at both ends, unlike Git's
`A..B` notation, and `--from` must be an ancestor of `--to`.

For example, to check the last three commits on the current branch:

```shell
cococheck --from HEAD~2
```

To check only the commits of a feature branch that are not on `main`, start at the oldest of them.
The branch must contain at least one such commit:

```shell
cococheck --from "$(git rev-list --reverse main..HEAD | head -n 1)"
```

### Output and exit status

The report of failed commits and the summary are written to standard output. Log messages enabled
with `--info` or `--debug`, and fatal errors, are written to standard error.

Each rule violation is reported with the name of the rule and, where possible, a snippet of the
commit message that marks the offending part:

```text
Commit 82074ea7070689f16ea531373fab9b997ecd69f6 failed:

error[BodyLineLength]: Body line 2 exceeds maximum length of 72 characters (actual: 82).
  |
4 | This body line is much too long, way more than seventy-two characters, to show it.
  | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
```

A commit that cannot be parsed is reported the same way, with a snippet that marks the problem:

```text
Commit c5d6497950f16a5d88d5b582db4b93180173a81d failed to parse:

error: invalid header: the scope must not be empty
  |
1 | feat(): Add a new feature
  |     ^^
```

When standard output is a terminal, the report is colored.

CoCoCheck exits with status `0` if all commits pass, and with status `1` if any commit fails or an
error occurs, for example because the repository or a revision cannot be found. This makes it
suitable for use in CI pipelines and Git hooks.

## Configuration

CoCoCheck reads its configuration from a file named `cococheck.yml` in the root of the repository
being checked. If the file does not exist, the defaults described below apply. Every section and
setting is optional; omitted values keep their defaults. Unknown keys are rejected, so a typo in a
rule name causes an error instead of being silently ignored.

The following file sets every option to its default value:

```yaml
parser:
  footer_detection: last_paragraph

rules:
  allowed_types:
    enabled: true
    allowed_types: [build, chore, ci, deps, docs, feat, fix, perf, refactor, revert, style, test]
  blank_line_before_footer:
    enabled: true
  body_line_length:
    enabled: true
    max_length: 72
  header_length:
    enabled: true
    max_length: 72
  no_description_terminal_punctuation:
    enabled: true
```

### Message structure

Independent of the configured rules, every commit message must have a valid Conventional Commits
header of the form `type(scope)!: description`, where the scope and the `!` are optional. If the
message has a body, the header must be followed by a blank line. A commit that does not meet
these requirements fails with a parse error, and no rules are applied to it.

### Parser

The `parser` section controls how CoCoCheck splits a message into header, body, and footers.

| Setting            | Description                                                        | Default          |
|--------------------|--------------------------------------------------------------------|------------------|
| `footer_detection` | Which paragraphs at the end of the message are treated as footers. | `last_paragraph` |

`footer_detection` accepts the following values:

- `last_paragraph`: Only the last paragraph can contain footers, and only if its first line is a
  footer.
- `trailing_paragraphs`: Every paragraph at the end of the message whose first line is a footer
  contains footers. Use this if footers in your repository are sometimes separated by blank lines.

A footer line consists of a token, followed by `: ` or ` #`, and a value, for example
`Refs: #123`, `Fixes #42`, or `BREAKING CHANGE: The API has changed`.

### Rules

All rules are enabled by default. To disable a rule, set its `enabled` setting to `false`:

```yaml
rules:
  body_line_length:
    enabled: false
```

#### `allowed_types`

Requires the commit type to be one of a fixed list of types.

| Setting         | Description                                                    | Default                                                                                              |
|-----------------|----------------------------------------------------------------|------------------------------------------------------------------------------------------------------|
| `enabled`       | Whether the rule is applied.                                   | `true`                                                                                               |
| `allowed_types` | The permitted types. A configured list replaces the default.   | `build`, `chore`, `ci`, `deps`, `docs`, `feat`, `fix`, `perf`, `refactor`, `revert`, `style`, `test` |

#### `blank_line_before_footer`

Requires footers to be separated from the body by a blank line. The rule reports a footer line
that appears inside the last paragraph of the body, after its first line, as in this message:

```text
feat: Add a new feature

This is the body.
Refs: #123
```

| Setting   | Description                  | Default |
|-----------|------------------------------|---------|
| `enabled` | Whether the rule is applied. | `true`  |

#### `body_line_length`

Limits the length of each line in the body. Lengths are counted in Unicode characters, not bytes.

| Setting      | Description                                | Default |
|--------------|--------------------------------------------|---------|
| `enabled`    | Whether the rule is applied.               | `true`  |
| `max_length` | The maximum number of characters per line. | `72`    |

#### `header_length`

Limits the length of the header line. Lengths are counted in Unicode characters, not bytes.

| Setting      | Description                                     | Default |
|--------------|-------------------------------------------------|---------|
| `enabled`    | Whether the rule is applied.                    | `true`  |
| `max_length` | The maximum number of characters in the header. | `72`    |

#### `no_description_terminal_punctuation`

Forbids the description in the header from ending with a period, an exclamation mark, or a
question mark.

| Setting   | Description                  | Default |
|-----------|------------------------------|---------|
| `enabled` | Whether the rule is applied. | `true`  |

## Project layout

The repository is a Cargo workspace with two crates:

- [`cococheck-core`](cococheck-core) contains the commit message parser, the configuration, and the
  rules.
- [`cococheck-cli`](cococheck-cli) contains the `cococheck` command-line tool.

## Contributing

Bug reports, feature requests, and pull requests are welcome. Please
[open an issue](https://github.com/mnonnenmacher/cococheck/issues) to discuss larger changes before
starting work on them.

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

The tests also run on the minimum supported Rust version, 1.89. If a change requires a newer Rust
version, update `rust-version` in [`Cargo.toml`](Cargo.toml) and the MSRV job in
[`.github/workflows/ci.yml`](.github/workflows/ci.yml) together.

Commit messages in this repository follow the Conventional Commits specification, so CoCoCheck
can check its own history.

## License

CoCoCheck is licensed under the [Apache License, Version 2.0](LICENSE).
