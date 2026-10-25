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

CoCoCheck requires Rust 1.89 or later. Install the latest version from the Git repository with
Cargo:

```shell
cargo install --git https://github.com/coco-tools/cococheck cococheck
```

This installs the `cococheck` binary into Cargo's binary directory, usually `~/.cargo/bin`.

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

CoCoCheck parses each commit message, applies the enabled [rules](#rules), and reports every
commit that fails:

```text
Commit 00e0da671f8fe523dc47c78033e776f803798424 failed to parse:

error: invalid header: expected ': ' after the type
  |
1 | Update stuff.
  |       ^

Commit 2819a1a133fa4e1fcefb08f90c1bb2bb8f675c93 failed:

error[no_description_terminal_punctuation]: Description should not end with terminal punctuation '.'.
  |
1 | fix(api): Handle errors.
  |                        ^

CoCoCheck finished checking 3 commits.
2 commits failed the checks.
```

### Options

| Argument or option      | Description                                                  | Default     |
|-------------------------|--------------------------------------------------------------|-------------|
| `[REVISION_RANGE]`      | A commit or a [range of commits](#selecting-commits).        | All commits |
| `-i`, `--input <INPUT>` | Path to the Git repository to check.                         | `.`         |
| `-c`, `--config <PATH>` | Path to the [configuration](#configuration) file.            | See below   |
| `--from <FROM>`         | Check only commits after this one. It is not checked itself. | All commits |
| `--to <TO>`             | Last commit to check. The commit itself is checked.          | `HEAD`      |
| `--from-latest-tag`     | Check only commits after the [latest tag](#latest-tag).      |             |
| `--message-file <FILE>` | Check the [message in a file](#commit-msg-hook).             |             |
| `--format <FORMAT>`     | Report as `text` or [`json`](#json-output).                  | `text`      |
| `--print-config`        | Print the [configuration](#effective-configuration) and exit |             |
|                         | without checking commits.                                    |             |
| `--color <WHEN>`        | [Color](#colors) the text report: `auto`, `always`, `never`. | `auto`      |
| `--deny-warnings`       | Fail commits that only have [warnings](#rules).              |             |
| `-v`, `--verbose`       | Log informational messages, such as passed commits. Give it  |             |
|                         | twice (`-vv`) to also log debug messages, such as the parsed |             |
|                         | commits.                                                     |             |
| `-h`, `--help`          | Print help.                                                  |             |
| `-V`, `--version`       | Print the version.                                           |             |

Without `--config`, CoCoCheck reads the [configuration file](#configuration) from the repository
given by `--input`, and uses the default configuration if there is none. A file given with
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
repository. A message is not checked if it is [ignored](#ignored-commits), for example the message
of a merge commit. To create a single commit without running the hook, use
`git commit --no-verify`.

### Output and exit status

The report of failed commits and the summary are written to standard output. Log messages enabled
with `-v` or `-vv`, and fatal errors, are written to standard error. Log messages start with
their level, like `info:` or `debug:`, so that they can be told apart from the report.
The `RUST_LOG` environment variable overrides the log level set with `-v`, for example
`RUST_LOG=debug`. See the
[`env_logger` documentation](https://docs.rs/env_logger/latest/env_logger/#enabling-logging) for
the syntax.

Each rule violation is reported with the name of the rule, as used in the
[configuration](#rules), and, where possible, a snippet of the commit message that marks the
offending part:

```text
Commit 82074ea7070689f16ea531373fab9b997ecd69f6 failed:

error[body_line_length]: Body line 2 exceeds maximum length of 72 characters (actual: 82).
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

| Field                         | Description                                                                                                         |
|-------------------------------|---------------------------------------------------------------------------------------------------------------------|
| `commits`                     | The checked commits, ordered from oldest to newest, including the ignored ones.                                     |
| `commits[].id`                | The commit ID, or `null` for a message checked with `--message-file`.                                               |
| `commits[].header`            | The first line of the commit message.                                                                               |
| `commits[].outcome`           | `passed`, `passed_with_warnings`, `failed`, or `ignored`.                                                           |
| `commits[].forbidden_pattern` | Why the message matches a [forbidden pattern](#forbidden-commits), with a `message` and a `span`, or `null`.        |
| `commits[].parse_error`       | Why the message could not be parsed, with a `message` and a `span`, or `null`. No rules are applied if it is set.   |
| `commits[].rules`             | The results of all enabled rules, or an empty list if the commit is ignored or could not be parsed.                 |
| `rules[].rule`                | The name of the rule, as used in the configuration.                                                                 |
| `rules[].severity`            | The configured severity of the rule, `error` or `warning`.                                                          |
| `rules[].violations`          | The violations of the rule, each with a `message` and a `span`. The rule passed if the list is empty.               |
| `span`                        | The byte range `start..end` in the full commit message that a violation refers to, or `null` if it has no location. |
| `summary`                     | The number of `checked` and `ignored` commits, and of the commits that `passed_with_warnings` or `failed`.          |

The exit status is the same as for the text report.

CoCoCheck exits with status `0` if all commits pass, and with status `1` if any commit fails or an
error occurs, for example because the repository or a revision cannot be found. Violations of rules
with the severity [`warning`](#rules) are reported, but do not cause a commit to fail, unless
`--deny-warnings` is given. This makes it suitable for use in CI pipelines and Git hooks.

## Configuration

CoCoCheck reads its configuration from a file in the root of the repository being checked, named
`cococheck.yml`, or `.cococheck.yml` to keep it hidden. The `.yaml` extension works as well. If
there is no such file, the defaults described below apply. If there is more than one, CoCoCheck
fails with an error instead of picking one. Every section and setting is optional; omitted values
keep their defaults. Unknown keys are rejected, so a typo in a rule name causes an error instead of
being silently ignored.

The following file sets every option to its default value:

```yaml
forbidden: []
default_ignores: true
ignores: []

parser:
  footer_detection: last_paragraph

rules:
  allowed_scopes:
    enabled: true
    severity: error
    scopes: []
  allowed_types:
    enabled: true
    severity: error
    types: [build, chore, ci, deps, docs, feat, fix, perf, refactor, revert, style, test]
  blank_line_before_footer:
    enabled: true
    severity: error
  body_length:
    enabled: true
    severity: error
    max_length: null
    min_length: null
  body_line_length:
    enabled: true
    severity: error
    max_length: 72
    ignore_urls: true
  description_case:
    enabled: true
    severity: error
    case: any
  header_length:
    enabled: true
    severity: error
    max_length: 72
  no_description_terminal_punctuation:
    enabled: true
    severity: error
    characters: .!?
  no_trailing_whitespace:
    enabled: true
    severity: error
  required_footers:
    enabled: true
    severity: error
    tokens: []
  scope_case:
    enabled: true
    severity: error
    case: lower
  scope_presence:
    enabled: true
    severity: error
    scope: optional
  type_case:
    enabled: true
    severity: error
```

### Effective configuration

To see which configuration CoCoCheck uses, pass `--print-config`. CoCoCheck then prints the
configuration as YAML, including the defaults of all omitted settings, and exits without checking
any commits. A comment in the first line names the file the configuration was loaded from, or says
that no file was found. The output is a valid configuration file, so it can also be used as a
starting point for a new one:

```shell
cococheck --print-config > cococheck.yml
```

`--print-config` respects `--input` and `--config`, but cannot be combined with the options that
select the commits to check.

### Message structure

Independent of the configured rules, every commit message must have a valid Conventional Commits
header of the form `type(scope)!: description`, where the scope and the `!` are optional. If the
message has a body, the header must be followed by a blank line. A commit that does not meet
these requirements fails with a parse error, and no rules are applied to it. Commits that are
[ignored](#ignored-commits) are not checked at all.

### Ignored commits

Git creates messages that do not follow the Conventional Commits specification, for example for
merge commits. CoCoCheck ignores commits with such messages by default. A message is ignored if it
starts with one of the following:

- `Merge branch `, `Merge branches `, `Merge tag `, `Merge tags `, `Merge commit `,
  `Merge remote-tracking branch `, or `Merge pull request `, as created by `git merge`, `git pull`,
  and GitHub or GitLab when merging a pull request.
- `Merged in ` or `Merged PR `, as created by Bitbucket and Azure DevOps when merging a pull
  request.
- `Revert "` or `Reapply "`, as created by `git revert`.
- `fixup! `, `squash! `, or `amend! `, as created by `git commit --fixup` and `git commit --squash`.

To ignore further commits, add [regular expressions](https://docs.rs/regex/latest/regex/#syntax) to
`ignores`. The expressions are matched against the whole message, without trailing whitespace, and
a message is ignored if any part of it matches one of them. `^` and `$` match at the start and end
of the message, so `^Initial commit$` only ignores messages that consist of this single line. To
make `^` and `$` match at the start and end of every line instead, add the `(?m)` flag:

```yaml
ignores:
  # Messages whose header starts with "WIP".
  - '^WIP\b'
  # Messages with a line that only contains "[skip checks]".
  - '(?m)^\[skip checks\]$'
```

To check the commits that are ignored by default, set `default_ignores` to `false`. The expressions
in `ignores` still apply.

| Setting           | Description                                                          | Default |
|-------------------|----------------------------------------------------------------------|---------|
| `default_ignores` | Whether the messages that Git creates, as listed above, are ignored. | `true`  |
| `ignores`         | Regular expressions for further messages to ignore.                  | `[]`    |

### Forbidden commits

Some commits are not meant to be merged, for example fixup commits, which are usually squashed
before merging, or commits that are marked as work in progress. To reject them, add
[regular expressions](https://docs.rs/regex/latest/regex/#syntax) to `forbidden`. They are matched
like the expressions in [`ignores`](#ignored-commits), but before them, so that a forbidden message
also fails if it is ignored, like the fixup commits that are ignored by default. A message that
matches one of them fails with a violation that marks the match, and no rules are applied to it:

```yaml
forbidden:
  # Commits created by `git commit --fixup` and `git commit --squash`.
  - '^(amend|fixup|squash)! '
  # Commits with "WIP" in any case as a word in the message.
  - '(?i)\bwip\b'
```

```text
Commit 2d2ab0e7ad0b4dbb08ab3e7b0b5c2a1fd5e5e0ea failed:

error: Message matches the forbidden pattern '^(amend|fixup|squash)! '.
  |
1 | fixup! feat: Add a feature
  | ^^^^^^^
```

The forbidden patterns also apply to the [`commit-msg` hook](#commit-msg-hook), so forbidding
fixup commits prevents creating them. To forbid them only in CI, keep them in a separate
configuration file that is passed with `--config` in CI.

| Setting     | Description                                         | Default |
|-------------|----------------------------------------------------|---------|
| `forbidden` | Regular expressions for messages that always fail. | `[]`    |

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
`Refs: #123`, `Fixes #42`, or `BREAKING CHANGE: The API has changed`. A token in brackets, like a
link reference `[1]: https://example.org`, must be followed by `: `, so that a numbered list like
`[1] #123` is not taken for footers.

### Rules

All rules are enabled by default. To disable a rule, set its `enabled` setting to `false`:

```yaml
rules:
  body_line_length:
    enabled: false
```

By default, a commit that violates a rule fails the checks. To report the violations of a rule
without failing the checks, set its `severity` setting to `warning`:

```yaml
rules:
  header_length:
    severity: warning
```

Warnings are reported like errors, but a commit that only has warnings passes, and CoCoCheck still
exits with status `0`.

To keep warnings advisory in the [`commit-msg` hook](#commit-msg-hook), but enforce them in CI,
pass `--deny-warnings` in CI. A commit with warnings then fails: It is reported as failed, with an
outcome of `failed` in the [JSON output](#json-output), and CoCoCheck exits with status `1`. The
violations are still reported as warnings.

#### `allowed_scopes`

Requires the scope to be one of a list of scopes. A header can name multiple scopes separated by
commas, like `feat(cli, core): Add a feature`, and each of them must be allowed. A commit without a
scope passes; use [`scope_presence`](#scope_presence) to require a scope. By default, the list is
empty and any scope is allowed.

| Setting    | Description                                                 | Default |
|------------|-------------------------------------------------------------|---------|
| `enabled`  | Whether the rule is applied.                                | `true`  |
| `severity` | Whether a violation is an `error` or a `warning`.           | `error` |
| `scopes`   | The permitted scopes. If it is empty, any scope is allowed. | `[]`    |

#### `allowed_types`

Requires the commit type to be one of a fixed list of types.

| Setting    | Description                                                  | Default                                                                                              |
|------------|--------------------------------------------------------------|------------------------------------------------------------------------------------------------------|
| `enabled`  | Whether the rule is applied.                                 | `true`                                                                                               |
| `severity` | Whether a violation is an `error` or a `warning`.            | `error`                                                                                              |
| `types`    | The permitted types. A configured list replaces the default. | `build`, `chore`, `ci`, `deps`, `docs`, `feat`, `fix`, `perf`, `refactor`, `revert`, `style`, `test` |

#### `blank_line_before_footer`

Requires footers to be separated from the body by a blank line. The rule reports a footer line
that appears inside the last paragraph of the body, after its first line, as in this message:

```text
feat: Add a new feature

This is the body.
Refs: #123
```

A paragraph that starts with a footer line is not reported, because it is a footer paragraph that
is followed by another one, which is parsed as body with the default `footer_detection` setting.
Use the `trailing_paragraphs` [footer detection](#parser) to parse it as footers. Lines in fenced
code blocks, which start and end with ```` ``` ```` or `~~~`, are not reported either.

To not report prose that looks like a footer, like the second line of
`There are multiple\nimplementations: One for files and one for databases.`, a footer line is only
reported if the line before it ends with `.`, `!`, or `?`, possibly followed by closing brackets or
quotes, ends a code block, or is a footer line itself. A footer after a line without punctuation,
like `This is the body\nRefs: #123`, is therefore not reported.

| Setting    | Description                                       | Default |
|------------|---------------------------------------------------|---------|
| `enabled`  | Whether the rule is applied.                      | `true`  |
| `severity` | Whether a violation is an `error` or a `warning`. | `error` |

#### `body_length`

Limits the length of the body as a whole, or requires a body, for example to make sure that every
commit explains why the change was made:

```yaml
rules:
  body_length:
    min_length: 1
```

Lengths are counted in Unicode characters, not bytes, and every line break counts as one character.
The footers are not part of the body, so a message that only has a `Signed-off-by` footer has no
body. The body is measured as parsed: trailing whitespace is removed from its lines, and multiple
blank lines between paragraphs count as one. A message without a body has a length of `0`. Both
limits are unset by default, so the rule passes every commit unless one of them is configured.

| Setting      | Description                                                       | Default |
|--------------|-------------------------------------------------------------------|---------|
| `enabled`    | Whether the rule is applied.                                      | `true`  |
| `severity`   | Whether a violation is an `error` or a `warning`.                 | `error` |
| `max_length` | The maximum number of characters in the body, or `null` for none. | `null`  |
| `min_length` | The minimum number of characters in the body, or `null` for none. | `null`  |
|              | `1` requires a body.                                              |         |

#### `body_line_length`

Limits the length of each line in the body. Lengths are counted in Unicode characters, not bytes.

Long URLs cannot be wrapped, so by default lines that contain a URL starting with `http://` or
`https://` are not checked.

| Setting       | Description                                       | Default |
|---------------|---------------------------------------------------|---------|
| `enabled`     | Whether the rule is applied.                      | `true`  |
| `severity`    | Whether a violation is an `error` or a `warning`. | `error` |
| `max_length`  | The maximum number of characters per line.        | `72`    |
| `ignore_urls` | Whether lines that contain a URL are skipped.     | `true`  |

#### `description_case`

Requires the description in the header to start with a lowercase or an uppercase letter. Only the
first character is checked, and a description that does not start with a letter, like
`` `--from` is exclusive ``, always passes. By default, any case is allowed.

`case` accepts the following values:

- `lower`: The description must start with a lowercase letter, like `feat: add a feature`.
- `sentence`: The description must start with an uppercase letter, like `feat: Add a feature`.
- `any`: The description can start with any letter.

| Setting    | Description                                       | Default |
|------------|---------------------------------------------------|---------|
| `enabled`  | Whether the rule is applied.                      | `true`  |
| `severity` | Whether a violation is an `error` or a `warning`. | `error` |
| `case`     | The required case of the first letter.            | `any`   |

#### `header_length`

Limits the length of the header line. Lengths are counted in Unicode characters, not bytes.

| Setting      | Description                                       | Default |
|--------------|---------------------------------------------------|---------|
| `enabled`    | Whether the rule is applied.                      | `true`  |
| `severity`   | Whether a violation is an `error` or a `warning`. | `error` |
| `max_length` | The maximum number of characters in the header.   | `72`    |

#### `no_description_terminal_punctuation`

Forbids the description in the header from ending with one of the configured characters. By
default, these are a period, an exclamation mark, and a question mark. Every character of
`characters` is forbidden, for example, to also forbid a comma, a semicolon, and a colon:

```yaml
rules:
  no_description_terminal_punctuation:
    characters: ".,;:!?"
```

As the description is the end of the header, this rule also limits how the header ends.

| Setting      | Description                                         | Default |
|--------------|-----------------------------------------------------|---------|
| `enabled`    | Whether the rule is applied.                        | `true`  |
| `severity`   | Whether a violation is an `error` or a `warning`.   | `error` |
| `characters` | The characters the description must not end with.   | `.!?`   |

#### `no_trailing_whitespace`

Forbids spaces, tabs, and other whitespace at the end of any line of the message, in the header,
the body, and the footers. Lines that only contain whitespace are reported as well. Each violation
marks the trailing whitespace.

With Git's default cleanup, trailing whitespace is removed before a commit is created, and
CoCoCheck removes it the same way for a message checked with
[`--message-file`](#commit-msg-hook). The rule therefore mainly reports commits in the history, or
messages created with `git commit --cleanup=verbatim`.

| Setting    | Description                                       | Default |
|------------|---------------------------------------------------|---------|
| `enabled`  | Whether the rule is applied.                      | `true`  |
| `severity` | Whether a violation is an `error` or a `warning`. | `error` |

#### `required_footers`

Requires every commit to have a footer with each of the configured tokens, for example a
`Signed-off-by` footer for the [Developer Certificate of Origin](https://developercertificate.org/):

```yaml
rules:
  required_footers:
    tokens: [Signed-off-by]
```

Tokens are compared case-insensitively, except for `BREAKING CHANGE` and `BREAKING-CHANGE`, which
must be uppercase. A footer line in the body, for example because it is not separated from the body
by a blank line, does not count.

| Setting    | Description                                       | Default |
|------------|---------------------------------------------------|---------|
| `enabled`  | Whether the rule is applied.                      | `true`  |
| `severity` | Whether a violation is an `error` or a `warning`. | `error` |
| `tokens`   | The tokens of the footers that are required.      | `[]`    |

#### `scope_case`

Requires the scope to be lowercase, so that `feat(Core): Add a feature` is reported. A commit
without a scope passes.

`case` accepts the following values:

- `lower`: The scope must not contain uppercase letters.
- `any`: The scope can contain letters of any case.

| Setting    | Description                                       | Default |
|------------|---------------------------------------------------|---------|
| `enabled`  | Whether the rule is applied.                      | `true`  |
| `severity` | Whether a violation is an `error` or a `warning`. | `error` |
| `case`     | The required case of the scope.                   | `lower` |

#### `scope_presence`

Requires every commit to have a scope, like `feat(core): Add a feature`, or forbids scopes.

`scope` accepts the following values:

- `optional`: A commit can have a scope, but does not have to.
- `required`: A commit must have a scope.
- `forbidden`: A commit must not have a scope.

| Setting    | Description                                       | Default    |
|------------|---------------------------------------------------|------------|
| `enabled`  | Whether the rule is applied.                      | `true`     |
| `severity` | Whether a violation is an `error` or a `warning`. | `error`    |
| `scope`    | Whether a scope is required, forbidden, or not.   | `optional` |

#### `type_case`

Requires the commit type to be lowercase, so that `Feat: Add a feature` is reported.

| Setting    | Description                                       | Default |
|------------|---------------------------------------------------|---------|
| `enabled`  | Whether the rule is applied.                      | `true`  |
| `severity` | Whether a violation is an `error` or a `warning`. | `error` |

## Project layout

The repository is a Cargo workspace with two crates:

- [`cococheck-core`](cococheck-core) contains the commit message parser, the configuration, and the
  rules.
- [`cococheck-cli`](cococheck-cli) contains the `cococheck` package with the command-line tool.

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

The tests also run on the minimum supported Rust version, 1.89. If a change requires a newer Rust
version, update `rust-version` in [`Cargo.toml`](Cargo.toml) and the MSRV job in
[`.github/workflows/ci.yml`](.github/workflows/ci.yml) together.

Commit messages in this repository follow the Conventional Commits specification. The CI pipeline
checks the commits of every pull request, and the commits pushed to `main`, with the CoCoCheck
version that contains them. To run the same check locally:

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
