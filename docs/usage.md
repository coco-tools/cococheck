# Usage

Run `cococheck` in a Git repository to check all commits up to `HEAD`:

```shell
cococheck
```

CoCoCheck parses each commit message, applies the enabled [rules](rules.md), and reports every
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

## Options

| Argument or option          | Description                                                        | Default     |
|-----------------------------|--------------------------------------------------------------------|-------------|
| `[REVISION_RANGE]`          | A commit or a [range of commits](#selecting-commits).              | All commits |
| `-r`, `--repository <PATH>` | Path to the Git repository to check.                               | `.`         |
| `-c`, `--config <PATH>`     | Path to the [configuration](configuration.md) file.                | See below   |
| `--from <FROM>`             | Check only commits after this one. It is not checked itself.       | All commits |
| `--to <TO>`                 | Last commit to check. The commit itself is checked.                | `HEAD`      |
| `--from-latest-tag`         | Check only commits after the [latest tag](#latest-tag).            |             |
| `--message-file <FILE>`     | Check the [message in a file](hooks.md#checking-a-message-file).   |             |
| `--format <FORMAT>`         | Report as `text` or [`json`](#json-output).                        | `text`      |
| `--print-config`            | Print the [configuration](configuration.md) and exit without       |             |
|                             | checking commits.                                                  |             |
| `--color <WHEN>`            | [Color](#colors) the text report: `auto`, `always`, `never`.       | `auto`      |
| `--deny-warnings`           | Fail commits that only have [warnings](rules.md).                  |             |
| `-v`, `--verbose`           | Log informational messages, such as passed commits. Give it twice  |             |
|                             | (`-vv`) to also log debug messages, such as the parsed commits.    |             |
| `-h`, `--help`              | Print help.                                                        |             |
| `-V`, `--version`           | Print the version.                                                 |             |

Without `--config`, CoCoCheck reads the [configuration file](configuration.md) from the repository
given by `--repository`, and uses the default configuration if there is none. A file given with
`--config` must exist; otherwise CoCoCheck fails with an error.

## Selecting commits

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

### Latest tag

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

## Output

The report of failed commits and the summary are written to standard output. Log messages enabled
with `-v` or `-vv`, and fatal errors, are written to standard error. Log messages start with
their level, like `info:` or `debug:`, so that they can be told apart from the report.
The `RUST_LOG` environment variable overrides the log level set with `-v`, for example
`RUST_LOG=debug`. See the
[`env_logger` documentation](https://docs.rs/env_logger/latest/env_logger/#enabling-logging) for
the syntax.

Each rule violation is reported with the name of the rule, as used in the
[configuration](rules.md), and, where possible, a snippet of the commit message that marks the
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

### Colors

By default, the text report is colored when standard output is a terminal. To choose explicitly,
pass `--color always` or `--color never`, for example `--color always` in a CI system like GitHub
Actions, which shows colors, but does not run CoCoCheck in a terminal.

With the default `--color auto`, the following environment variables are also respected:

- [`NO_COLOR`](https://no-color.org/): If it is set to a non-empty value, the report is not
  colored.
- [`CLICOLOR_FORCE`](https://bixense.com/clicolors/): If it is set to a value other than `0`, the
  report is colored, even if standard output is not a terminal. `NO_COLOR` takes precedence.

`--color always` and `--color never` override both variables.

## JSON output

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
| `commits[].forbidden_pattern` | Why the message matches a [forbidden pattern](configuration.md#forbidden-commits), with a `message` and a `span`, or `null`. |
| `commits[].parse_error`       | Why the message could not be parsed, with a `message` and a `span`, or `null`. No rules are applied if it is set.                 |
| `commits[].rules`             | The results of all enabled rules, or an empty list if the commit is ignored, matches a forbidden pattern, or could not be parsed. |
| `rules[].rule`                | The name of the rule, as used in the configuration.                                                                               |
| `rules[].severity`            | The configured severity of the rule, `error` or `warning`.                                                                        |
| `rules[].violations`          | The violations of the rule, each with a `message` and a `span`. The rule passed if the list is empty.                             |
| `span`                        | The byte range `start..end` in the full commit message that a violation refers to, or `null` if it has no location.               |
| `summary`                     | The number of `checked` and `ignored` commits, and of the commits that `passed_with_warnings` or `failed`.                        |

## Exit status

CoCoCheck exits with one of the following statuses, for the text report and the JSON output alike,
so that CI pipelines and Git hooks can tell failed commits apart from errors:

| Status | Meaning                                                                                       |
|--------|-----------------------------------------------------------------------------------------------|
| `0`    | All commits passed the checks.                                                                |
| `1`    | At least one commit failed the checks.                                                        |
| `2`    | An error occurred, for example because of invalid arguments, an invalid configuration, or a repository or a revision that cannot be found. |

Violations of rules with the severity [`warning`](rules.md) are reported, but do not cause a
commit to fail, unless `--deny-warnings` is given.
