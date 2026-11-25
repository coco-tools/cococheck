# CoCoCheck

[![CI](https://github.com/coco-tools/cococheck/actions/workflows/ci.yml/badge.svg)](https://github.com/coco-tools/cococheck/actions/workflows/ci.yml)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

CoCoCheck checks that the commit messages in a Git repository follow the
[Conventional Commits](https://www.conventionalcommits.org/) specification. It is fast and
self-contained, written in Rust and distributed as a single binary with no runtime dependencies.

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

### Prebuilt binaries

Every [release](https://github.com/coco-tools/cococheck/releases) provides prebuilt binaries for the
following platforms, which have no dependencies and do not require Rust:

| Platform | Architecture | Archive                                       |
|----------|--------------|-----------------------------------------------|
| Linux    | x86_64       | `cococheck-x86_64-unknown-linux-musl.tar.gz`  |
| Linux    | aarch64      | `cococheck-aarch64-unknown-linux-musl.tar.gz` |
| macOS    | aarch64      | `cococheck-aarch64-apple-darwin.tar.gz`       |
| macOS    | x86_64       | `cococheck-x86_64-apple-darwin.tar.gz`        |
| Windows  | x86_64       | `cococheck-x86_64-pc-windows-msvc.zip`        |

Download the archive for your platform from the
[latest release](https://github.com/coco-tools/cococheck/releases/latest), extract it, and move the
`cococheck` binary to a directory in your `PATH`. For example, on Linux on x86_64:

```shell
curl -sSfL https://github.com/coco-tools/cococheck/releases/latest/download/cococheck-x86_64-unknown-linux-musl.tar.gz \
  | tar -xz -C ~/.local/bin cococheck
```

Every archive comes with a `.sha256` file with its SHA-256 checksum.

With [cargo-binstall](https://github.com/cargo-bins/cargo-binstall), `cargo binstall cococheck`
downloads the binary for your platform and installs it into Cargo's binary directory.

### From source

Building CoCoCheck requires Rust 1.89 or later. Install the latest release from
[crates.io](https://crates.io/crates/cococheck) with Cargo:

```shell
cargo install cococheck
```

This installs the `cococheck` binary into Cargo's binary directory, usually `~/.cargo/bin`.

To install the development version from the Git repository instead:

```shell
cargo install --git https://github.com/coco-tools/cococheck cococheck
```

To build from source instead, clone the repository and build the release binary:

```shell
git clone https://github.com/coco-tools/cococheck.git
cd cococheck
cargo build --release
```

The binary is written to `target/release/cococheck`.

## Usage

Run `cococheck` in a Git repository to check all commits up to `HEAD`:

```shell
cococheck
```

CoCoCheck parses each commit message, applies the enabled [rules](docs/rules.md), and reports every
commit that fails:

```text
Commit 1/3 00e0da6 "Update stuff." failed to parse:

error: invalid header: expected ': ' after the type
  |
1 | Update stuff.
  |       ^

Commit 3/3 2819a1a "fix(api): Handle errors." failed:

error[no_description_terminal_punctuation]: Description should not end with terminal punctuation '.'.
  |
1 | fix(api): Handle errors.
  |                        ^

CoCoCheck finished checking 3 commits.
2 commits failed the checks.
```

### Options

| Argument or option          | Description                                                  | Default     |
|-----------------------------|--------------------------------------------------------------|-------------|
| `[REVISION_RANGE]`          | A commit or a [range of commits](#selecting-commits).        | All commits |
| `-r`, `--repository <PATH>` | Path to the Git repository to check.                         | `.`         |
| `-c`, `--config <PATH>`     | Path to the [configuration](#configuration) file.            | See below   |
| `--from <FROM>`             | Check only commits after this one. It is not checked itself. | All commits |
| `--to <TO>`                 | Last commit to check. The commit itself is checked.          | `HEAD`      |
| `--from-latest-tag`         | Check only commits after the [latest tag](#latest-tag).      |             |
| `--message-file <FILE>`     | Check the [message in a file](#commit-msg-hook).             |             |
| `--format <FORMAT>`         | Report as `text` or [`json`](#json-output).                  | `text`      |
| `--print-config`            | Print the [configuration](#configuration) and exit           |             |
|                             | without checking commits.                                    |             |
| `--color <WHEN>`            | [Color](#colors) the text report: `auto`, `always`, `never`. | `auto`      |
| `--deny-warnings`           | Fail commits that only have [warnings](docs/rules.md).       |             |
| `-v`, `--verbose`           | Log informational messages, such as passed commits. Give it  |             |
|                             | twice (`-vv`) to also log debug messages, such as the parsed |             |
|                             | commits.                                                     |             |
| `-h`, `--help`              | Print help.                                                  |             |
| `-V`, `--version`           | Print the version.                                           |             |

Without `--config`, CoCoCheck reads the [configuration file](#configuration) from the repository
given by `--repository`, and uses the default configuration if there is none. A file given with
`--config` must exist; otherwise CoCoCheck fails with an error.

### Selecting commits

Without arguments, CoCoCheck checks all commits up to `HEAD`. To check only some commits, pass a
revision or a revision range, like for `git show`:

| Argument   | Checked commits                                              |
|------------|--------------------------------------------------------------|
| `HEAD`     | Only the commit `HEAD`.                                      |
| `HEAD~3..` | The last three commits, `HEAD~2` to `HEAD`.                  |
| `main..`   | The commits of the current branch that are not on `main`.    |
| `v1..v2`   | The commits that are reachable from `v2`, but not from `v1`. |

A revision is anything that Git resolves to a commit, such as a commit ID, a branch or tag name, or
an expression like `HEAD~3`. Note that unlike for `git log`, a single revision selects only that
commit, not all commits up to it.

Like in Git, the range `A..B` contains the commits that are reachable from `B`, but not from `A`, so
`A` itself is not checked. An omitted end of the range means `HEAD`. `A` does not have to be an
ancestor of `B`, so a branch can be checked against a `main` branch that has moved on since the
branch was created. If the range is empty, for example because both ends are the same commit, the
check passes. The symmetric difference `A...B` is not supported.

Alternatively, the ends of the range can be given with `--from` and `--to`: `--from A --to B` is the
same as `A..B`. Both are optional, so `--to B` checks all commits up to `B`, which cannot be
expressed with a revision range. `--from` and `--to` cannot be combined with a revision range.

#### Latest tag

To check the commits that go into the next release, pass `--from-latest-tag`. It works like
`--from` with the latest tag that is reachable from `--to`, or from `HEAD` if `--to` is not given,
so the following commands are equivalent:

```shell
cococheck --from-latest-tag
cococheck "$(git describe --tags --abbrev=0)..HEAD"
```

Like for `git describe --tags`, the latest tag is the one with the fewest commits between it and
`--to`, and both annotated and lightweight tags count. If `--to` itself is tagged, there are no
commits to check. If no tag is reachable, for example before the first release, all commits up to
`--to` are checked. `--from-latest-tag` cannot be combined with `--from` or a revision range.

### `commit-msg` hook

To check a commit message before the commit is created, pass the file that contains the message
with `--message-file`, or `-` to read the message from standard input. This cannot be combined with
a revision range, `--from`, `--to`, or `--from-latest-tag`.

Like Git, CoCoCheck cleans up the message before checking it: It removes comment lines and, for
`git commit --verbose`, the scissors line and the diff below it, and removes surplus whitespace.
Comment lines start with the string configured in Git's `core.commentString` or `core.commentChar`
setting, or `#` if it is not set or set to `auto`. If Git's `commit.cleanup` setting is
`scissors`, `whitespace`, or `verbatim`, CoCoCheck cleans up the message in the same way.

A hook cannot detect a `--cleanup` option passed to `git commit`, or whether the message was
written in an editor. In Git's default cleanup mode, comment lines are only removed if the message
was edited, so a line of a message given with `git commit -m` that starts with the comment string
is part of the commit, but is not checked.

To use CoCoCheck as a [`commit-msg` hook](https://git-scm.com/docs/githooks#_commit_msg), which
rejects commits whose message fails the checks, create the file `.git/hooks/commit-msg` with the
following content and make it executable with `chmod +x .git/hooks/commit-msg`:

```shell
#!/bin/sh
exec cococheck --message-file "$1"
```

Git runs the hook in the root of the working tree, so CoCoCheck reads the configuration file of the
repository. A message is not checked if it is [ignored](docs/configuration.md#ignored-commits), for
example the message of a merge commit. To create a single commit without running the hook, use
`git commit --no-verify`.

#### Hook managers

Instead of writing the hook by hand, it can be set up with a hook manager. This shares the hook
with everyone who works on the repository.

With [pre-commit](https://pre-commit.com/), add CoCoCheck to the `.pre-commit-config.yaml`, and set
`rev` to the commit ID of the CoCoCheck version to use:

```yaml
default_install_hook_types: [pre-commit, commit-msg]
repos:
  - repo: https://github.com/coco-tools/cococheck
    rev: <commit ID>
    hooks:
      - id: cococheck
```

Then install the hooks with `pre-commit install`. The `default_install_hook_types` setting makes
it install the `commit-msg` hook, which pre-commit does not install by default. Without it, run
`pre-commit install --hook-type commit-msg`. The hook builds CoCoCheck from source with Cargo
when it is run for the first time, so it requires a [Rust toolchain](#installation).

With [Lefthook](https://lefthook.dev/), add CoCoCheck to the `lefthook.yml`, and install the hooks
with `lefthook install`:

```yaml
commit-msg:
  commands:
    cococheck:
      run: cococheck --message-file {1}
```

With [husky](https://typicode.github.io/husky/), create the file `.husky/commit-msg` with the
following content:

```shell
cococheck --message-file "$1"
```

Lefthook and husky run the `cococheck` binary from the `PATH`, so it must be
[installed](#installation) first.

### Output and exit status

The report of failed commits and the summary are written to standard output. Log messages enabled
with `-v` or `-vv`, and fatal errors, are written to standard error. Log messages start with
their level, like `info:` or `debug:`, so that they can be told apart from the report.
The `RUST_LOG` environment variable overrides the log level set with `-v`, for example
`RUST_LOG=debug`. See the
[`env_logger` documentation](https://docs.rs/env_logger/latest/env_logger/#enabling-logging) for
the syntax.

Each rule violation is reported with the name of the rule, as used in the
[configuration](docs/rules.md), and, where possible, a snippet of the commit message that marks the
offending part:

```text
Commit 1/1 82074ea "docs: Describe the body rules" failed:

error[body_line_length]: Body line 2 exceeds maximum length of 72 characters (actual: 82).
  |
4 | This body line is much too long, way more than seventy-two characters, to show it.
  | ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
```

A commit that cannot be parsed is reported the same way, with a snippet that marks the problem:

```text
Commit 1/1 c5d6497 "feat(): Add a new feature" failed to parse:

error: invalid header: the scope must not be empty
  |
1 | feat(): Add a new feature
  |     ^^
```

#### Colors

By default, the text report is colored when standard output is a terminal. To choose explicitly,
pass `--color always` or `--color never`, for example `--color always` in a CI system like GitHub
Actions, which shows colors, but does not run CoCoCheck in a terminal.

With the default `--color auto`, the following environment variables are also respected:

- [`NO_COLOR`](https://no-color.org/): If it is set to a non-empty value, the report is not
  colored.
- [`CLICOLOR_FORCE`](https://bixense.com/clicolors/): If it is set to a value other than `0`, the
  report is colored, even if standard output is not a terminal. `NO_COLOR` takes precedence.

`--color always` and `--color never` override both variables.

### JSON output

To process the results in other tools, for example in a CI integration, pass `--format json`.
CoCoCheck then writes a single JSON document to standard output instead of the text report:

```json
{
  "commits": [
    {
      "id": "2819a1a133fa4e1fcefb08f90c1bb2bb8f675c93",
      "header": "fix(api): Handle errors.",
      "outcome": "failed",
      "forbidden_pattern": null,
      "parse_error": null,
      "rules": [
        {
          "rule": "no_description_terminal_punctuation",
          "severity": "error",
          "violations": [
            {
              "message": "Description should not end with terminal punctuation '.'.",
              "span": { "start": 23, "end": 24 }
            }
          ]
        }
      ]
    }
  ],
  "summary": { "checked": 1, "ignored": 0, "passed_with_warnings": 0, "failed": 1 }
}
```

The example shows only one rule; the report lists every enabled rule.

| Field                         | Description                                                                                                                       |
|-------------------------------|-----------------------------------------------------------------------------------------------------------------------------------|
| `commits`                     | The checked commits, ordered from oldest to newest, including the ignored ones.                                                   |
| `commits[].id`                | The commit ID, or `null` for a message checked with `--message-file`.                                                             |
| `commits[].header`            | The first line of the commit message.                                                                                             |
| `commits[].outcome`           | `passed`, `passed_with_warnings`, `failed`, or `ignored`.                                                                         |
| `commits[].forbidden_pattern` | Why the message matches a [forbidden pattern](docs/configuration.md#forbidden-commits), with a `message` and a `span`, or `null`. |
| `commits[].parse_error`       | Why the message could not be parsed, with a `message` and a `span`, or `null`. No rules are applied if it is set.                 |
| `commits[].rules`             | The results of all enabled rules, or an empty list if the commit is ignored, matches a forbidden pattern, or could not be parsed. |
| `rules[].rule`                | The name of the rule, as used in the configuration.                                                                               |
| `rules[].severity`            | The configured severity of the rule, `error` or `warning`.                                                                        |
| `rules[].violations`          | The violations of the rule, each with a `message` and a `span`. The rule passed if the list is empty.                             |
| `span`                        | The byte range `start..end` in the full commit message that a violation refers to, or `null` if it has no location.               |
| `summary`                     | The number of `checked` and `ignored` commits, and of the commits that `passed_with_warnings` or `failed`.                        |

The exit status is the same as for the text report.

CoCoCheck exits with one of the following statuses, so that CI pipelines and Git hooks can tell
failed commits apart from errors:

| Status | Meaning                                                                                       |
|--------|-----------------------------------------------------------------------------------------------|
| `0`    | All commits passed the checks.                                                                |
| `1`    | At least one commit failed the checks.                                                        |
| `2`    | An error occurred, for example because of invalid arguments, an invalid configuration, or a repository or a revision that cannot be found. |

Violations of rules with the severity [`warning`](docs/rules.md) are reported, but do not cause a
commit to fail, unless `--deny-warnings` is given.

## Configuration

CoCoCheck reads its configuration from a file named `cococheck.yml` or `.cococheck.yml` in the root
of the repository being checked; the `.yaml` extension works as well. Without such a file, the
defaults apply: all rules are enabled, with limits like a maximum header length of 72 characters,
and the messages that Git creates for merge, revert, and fixup commits are ignored. Every setting is
optional, so a configuration file only needs to contain what differs from the defaults:

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
`cococheck --print-config`. The full reference is in the [`docs`](docs) directory:

- [Configuration](docs/configuration.md): The configuration file, ignored and forbidden commits, and
  the parser.
- [Rules](docs/rules.md): All rules, their settings, and how to change their severity.

## Project layout

The repository is a Cargo workspace with two crates:

- The root directory contains the `cococheck` package with the command-line tool. It is in the
  root, so that [pre-commit](#hook-managers) can install it from the repository.
- [`cococheck-core`](cococheck-core) contains the commit message parser, the configuration, and the
  rules.

The [`docs`](docs) directory contains the reference of the configuration and the rules, and the
description of the [release process](docs/releasing.md) for maintainers.

## Contributing

Bug reports, feature requests, and pull requests are welcome. Please
[open an issue](https://github.com/coco-tools/cococheck/issues) to discuss larger changes before
starting work on them.

Please report security vulnerabilities privately as described in [`SECURITY.md`](SECURITY.md).

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

Commit messages in this repository follow the Conventional Commits specification. The CI pipeline
checks the commits of every pull request, and the commits pushed to `main`, with the
[CoCoCheck action](https://github.com/coco-tools/cococheck-action), which uses a released version of
CoCoCheck. A change to CoCoCheck only affects this check once it is released and adopted by a new
release of the action. To run the check locally with the version of your branch:

```shell
cargo run -p cococheck -- main..
```

### Developer Certificate of Origin

By contributing to this project, you certify that you have the right to submit your contribution
under the project's license, as described in the
[Developer Certificate of Origin](https://developercertificate.org/) (DCO). To certify it, add a
`Signed-off-by` footer with your name and email address to every commit, which Git does when you
commit with `git commit -s`:

```text
Signed-off-by: Jane Doe <jane.doe@example.org>
```

The CI pipeline rejects commits without this footer.

## License

CoCoCheck is licensed under the [Apache License, Version 2.0](LICENSE).
