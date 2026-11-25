# Configuration

CoCoCheck reads its configuration from a file in the root of the repository being checked, named
`cococheck.yml`, or `.cococheck.yml` to keep it hidden. The `.yaml` extension works as well. If
there is no such file, the defaults described below apply. If there is more than one, CoCoCheck
fails with an error instead of picking one. Every section and setting is optional; omitted values
keep their defaults. Unknown keys are rejected, so a typo in a rule name or a setting causes an
error instead of being silently ignored.

The following file sets every option to its default value:

```yaml
forbidden: []
default_ignores: true
ignore_merge_commits: true
ignores: []
ignore_authors: []

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
  breaking_change:
    enabled: true
    severity: error
    footer_required: false
    marker_required: true
  description_case:
    enabled: true
    severity: error
    case: any
  footer_length:
    enabled: true
    severity: error
    max_length: null
  footer_line_length:
    enabled: true
    severity: error
    max_length: null
    ignore_urls: true
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

## Effective configuration

To see which configuration CoCoCheck uses, pass `--print-config`. CoCoCheck then prints the
configuration as YAML, including the defaults of all omitted settings, and exits without checking
any commits. A comment in the first line names the file the configuration was loaded from, or says
that no file was found. The output is a valid configuration file, so it can also be used as a
starting point for a new one:

```shell
cococheck --print-config > cococheck.yml
```

`--print-config` respects `--repository` and `--config`, but cannot be combined with the options that
select the commits to check.

## Message structure

Independent of the configured rules, every commit message must have a valid Conventional Commits
header of the form `type(scope)!: description`, where the scope and the `!` are optional. If the
message has a body, the header must be followed by a blank line. A commit that does not meet
these requirements fails with a parse error, and no rules are applied to it. Commits that are
[ignored](#ignored-commits) are neither parsed nor checked by the rules, but they still fail if
they match a [forbidden pattern](#forbidden-commits).

## Ignored commits

Git, Git hosting platforms, and release tools create messages that do not follow the Conventional
Commits specification, for example for merge commits. CoCoCheck ignores commits with such messages
by default. A message is ignored if it starts with one of the following:

- `Merge branch `, `Merge branches `, `Merge tag `, `Merge tags `, `Merge commit `,
  `Merge remote-tracking branch `, or `Merge pull request `, as created by `git merge`, `git pull`,
  and GitHub or GitLab when merging a pull request.
- `Merge X into Y`, `Merged X into Y`, `Merged in `, `Merged PR `, `Automatic merge`, or
  `Auto-merged X into Y`, as created by Bitbucket, Azure DevOps, and other tools when merging a
  pull request or branch. `into` must be in the first line.
- `Revert "` or `Reapply "`, as created by `git revert`.
- `fixup! `, `squash! `, or `amend! `, as created by `git commit --fixup` and `git commit --squash`.

A message is also ignored if it only consists of a [semantic version](https://semver.org/), with an
optional `v` prefix, like `1.2.3` or `v1.2.3-rc.1`, as created by `npm version`.

CoCoCheck does not ignore messages like `Revert the last change` or `revert: Remove a feature`,
because they are written by hand, not created by Git.

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

Merge commits, which have more than one parent, are also ignored regardless of their message, so
that a merge commit with a custom message, for example from `git merge -m`, does not fail the
checks. To check their messages, set `ignore_merge_commits` to `false`; merge commits whose message
matches one of the patterns above are still ignored. In the
[`commit-msg` hook](../README.md#commit-msg-hook), the commit does not exist yet, so
`ignore_merge_commits` does not apply, but the default message of `git merge` is ignored by
`default_ignores`.

Commits by bots like Dependabot or Renovate often have messages that a project cannot easily
change. To ignore commits by their author, add
[regular expressions](https://docs.rs/regex/latest/regex/#syntax) to `ignore_authors`. They are
matched against the author as `Name <email>`, and a commit is ignored if any part of its author
matches one of them:

```yaml
ignore_authors:
  - '^dependabot\[bot\] '
  - '^renovate\[bot\] '
```

In the [`commit-msg` hook](../README.md#commit-msg-hook), the commit does not exist yet, so
CoCoCheck determines its author with `git var GIT_AUTHOR_IDENT`. Like Git, this respects the
`GIT_AUTHOR_NAME` and `GIT_AUTHOR_EMAIL` environment variables and the `author.*` and `user.*`
settings. `git commit` passes the author to its hooks in these variables, so the `--author` option
and the author of an amended commit are also respected.

| Setting                | Description                                               | Default |
|------------------------|-----------------------------------------------------------|---------|
| `default_ignores`      | Whether the messages listed above are ignored.            | `true`  |
| `ignore_merge_commits` | Whether commits with more than one parent are ignored.    | `true`  |
| `ignores`              | Regular expressions for further messages to ignore.       | `[]`    |
| `ignore_authors`       | Regular expressions for the authors of commits to ignore. | `[]`    |

## Forbidden commits

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
Commit 1/1 2d2ab0e "fixup! feat: Add a feature" failed:

error: Message matches the forbidden pattern '^(amend|fixup|squash)! '.
  |
1 | fixup! feat: Add a feature
  | ^^^^^^^
```

The forbidden patterns also apply to the [`commit-msg` hook](../README.md#commit-msg-hook), so
forbidding fixup commits prevents creating them. To forbid them only in CI, keep them in a separate
configuration file that is passed with `--config` in CI.

| Setting     | Description                                        | Default |
|-------------|----------------------------------------------------|---------|
| `forbidden` | Regular expressions for messages that always fail. | `[]`    |

## Parser

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
