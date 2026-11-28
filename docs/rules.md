# Rules

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

To keep warnings advisory in the [`commit-msg` hook](hooks.md), but enforce them
in CI, pass `--deny-warnings` in CI. A commit with warnings then fails: It is reported as failed,
with an outcome of `failed` in the [JSON output](usage.md#json-output), and CoCoCheck exits with
status `1`. The violations are still reported as warnings.

## Ignoring commits for a rule

Some commits cannot meet a single rule, but should still be checked by all others. For example,
release tools and bots often create commits without a `Signed-off-by` footer. To skip a rule for
such commits, add [regular expressions](https://docs.rs/regex/latest/regex/#syntax) to the
`ignores` or `ignore_authors` setting of the rule:

```yaml
rules:
  required_footers:
    tokens: [Signed-off-by]
    ignores:
      # The commits of release pull requests.
      - '^chore: Release v'
    ignore_authors:
      - '^renovate\[bot\] '
```

Every rule has these settings. They are matched like the
[global settings of the same name](configuration.md#ignored-commits): `ignores` against the whole
message without trailing whitespace, and `ignore_authors` against the author as `Name <email>`.
The rule does not check a commit if any part of its message or author matches one of them, and the
commit is not listed in the results of the rule in the
[JSON output](usage.md#json-output). The commit still fails if it matches a
[forbidden pattern](configuration.md#forbidden-commits) or cannot be parsed. To skip all rules for
a commit, use the global settings instead.

| Setting          | Description                                                                 | Default |
|------------------|-----------------------------------------------------------------------------|---------|
| `ignores`        | Regular expressions for the messages that the rule does not check.          | `[]`    |
| `ignore_authors` | Regular expressions for the authors of the commits the rule does not check. | `[]`    |

## Available rules

The following rules are available:

- [`allowed_scopes`](#allowed_scopes)
- [`allowed_types`](#allowed_types)
- [`blank_line_before_footer`](#blank_line_before_footer)
- [`body_length`](#body_length)
- [`body_line_length`](#body_line_length)
- [`breaking_change`](#breaking_change)
- [`description_case`](#description_case)
- [`footer_length`](#footer_length)
- [`footer_line_length`](#footer_line_length)
- [`header_length`](#header_length)
- [`no_description_terminal_punctuation`](#no_description_terminal_punctuation)
- [`no_trailing_whitespace`](#no_trailing_whitespace)
- [`required_footers`](#required_footers)
- [`scope_case`](#scope_case)
- [`scope_presence`](#scope_presence)
- [`type_case`](#type_case)

## `allowed_scopes`

Requires the scope to be one of a list of scopes. A header can name multiple scopes separated by
commas, like `feat(cli, core): Add a feature`, and each of them must be allowed. A commit without a
scope passes; use [`scope_presence`](#scope_presence) to require a scope. By default, the list is
empty and any scope is allowed.

| Setting    | Description                                                 | Default |
|------------|-------------------------------------------------------------|---------|
| `enabled`  | Whether the rule is applied.                                | `true`  |
| `severity` | Whether a violation is an `error` or a `warning`.           | `error` |
| `scopes`   | The permitted scopes. If it is empty, any scope is allowed. | `[]`    |

## `allowed_types`

Requires the commit type to be one of a fixed list of types.

| Setting    | Description                                                  | Default                                                                                              |
|------------|--------------------------------------------------------------|------------------------------------------------------------------------------------------------------|
| `enabled`  | Whether the rule is applied.                                 | `true`                                                                                               |
| `severity` | Whether a violation is an `error` or a `warning`.            | `error`                                                                                              |
| `types`    | The permitted types. A configured list replaces the default. | `build`, `chore`, `ci`, `deps`, `docs`, `feat`, `fix`, `perf`, `refactor`, `revert`, `style`, `test` |

## `blank_line_before_footer`

Requires footers to be separated from the body by a blank line. The rule reports a footer line
that appears inside the last paragraph of the body, after its first line, as in this message:

```text
feat: Add a new feature

This is the body.
Refs: #123
```

A paragraph that starts with a footer line is not reported, because it is a footer paragraph that is
followed by another one, which is parsed as body with the default `footer_detection` setting. Use
the `trailing_paragraphs` [footer detection](configuration.md#parser) to parse it as footers. Lines
in fenced code blocks, which start and end with ```` ``` ```` or `~~~`, are not reported either.

To not report prose that looks like a footer, like the second line of
`There are multiple\nimplementations: One for files and one for databases.`, a footer line is only
reported if the line before it ends with `.`, `!`, or `?`, possibly followed by closing brackets or
quotes, ends a code block, or is a footer line itself. A footer after a line without punctuation,
like `This is the body\nRefs: #123`, is therefore not reported.

| Setting    | Description                                       | Default |
|------------|---------------------------------------------------|---------|
| `enabled`  | Whether the rule is applied.                      | `true`  |
| `severity` | Whether a violation is an `error` or a `warning`. | `error` |

## `body_length`

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

## `body_line_length`

Limits the length of each line in the body. Lengths are counted in Unicode characters, not bytes.

Long URLs cannot be wrapped, so by default lines that contain a URL starting with `http://` or
`https://` are not checked. The footers are not part of the body; their lines are checked by
[`footer_line_length`](#footer_line_length).

| Setting       | Description                                       | Default |
|---------------|---------------------------------------------------|---------|
| `enabled`     | Whether the rule is applied.                      | `true`  |
| `severity`    | Whether a violation is an `error` or a `warning`. | `error` |
| `max_length`  | The maximum number of characters per line.        | `72`    |
| `ignore_urls` | Whether lines that contain a URL are skipped.     | `true`  |

## `breaking_change`

The Conventional Commits specification allows marking a breaking change with a `!` before the `:`
in the header, with a `BREAKING CHANGE` or `BREAKING-CHANGE` footer, or with both. This rule
requires a consistent style. By default, a footer requires a `!`, so that breaking changes are
visible in one-line logs. To also require a footer that describes the change if the header has a
`!`:

```yaml
rules:
  breaking_change:
    footer_required: true
```

Like the other footers, a `BREAKING CHANGE` footer is only recognized after a blank line, at the end
of the message, as described for the [parser](configuration.md#parser). To allow a footer without a
`!`, set `marker_required` to `false`.

| Setting           | Description                                                      | Default |
|-------------------|------------------------------------------------------------------|---------|
| `enabled`         | Whether the rule is applied.                                     | `true`  |
| `severity`        | Whether a violation is an `error` or a `warning`.                | `error` |
| `footer_required` | Whether a `!` in the header requires a `BREAKING CHANGE` footer. | `false` |
| `marker_required` | Whether a `BREAKING CHANGE` footer requires a `!` in the header. | `true`  |

## `description_case`

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

## `footer_length`

Limits the length of all footers together. They are measured like the body by
[`body_length`](#body_length): lengths are counted in Unicode characters, every line break counts as
one character, and multiple blank lines between footer paragraphs count as one. The limit is unset
by default, so the rule passes every commit unless it is configured.

| Setting      | Description                                                          | Default |
|--------------|----------------------------------------------------------------------|---------|
| `enabled`    | Whether the rule is applied.                                         | `true`  |
| `severity`   | Whether a violation is an `error` or a `warning`.                    | `error` |
| `max_length` | The maximum number of characters of all footers, or `null` for none. | `null`  |

## `footer_line_length`

Limits the length of each line of the footers. Lengths are counted in Unicode characters, not bytes.
Like in [`body_line_length`](#body_line_length), lines that contain a URL are not checked by
default, because footers like `Refs: https://…` cannot be wrapped. The limit is unset by default, so
the rule passes every commit unless it is configured:

```yaml
rules:
  footer_line_length:
    max_length: 72
```

| Setting       | Description                                                    | Default |
|---------------|----------------------------------------------------------------|---------|
| `enabled`     | Whether the rule is applied.                                   | `true`  |
| `severity`    | Whether a violation is an `error` or a `warning`.              | `error` |
| `max_length`  | The maximum number of characters per line, or `null` for none. | `null`  |
| `ignore_urls` | Whether lines that contain a URL are skipped.                  | `true`  |

## `header_length`

Limits the length of the header line. Lengths are counted in Unicode characters, not bytes.

| Setting      | Description                                       | Default |
|--------------|---------------------------------------------------|---------|
| `enabled`    | Whether the rule is applied.                      | `true`  |
| `severity`   | Whether a violation is an `error` or a `warning`. | `error` |
| `max_length` | The maximum number of characters in the header.   | `72`    |

## `no_description_terminal_punctuation`

Forbids the description in the header from ending with one of the configured characters. By
default, these are a period, an exclamation mark, and a question mark. Every character of
`characters` is forbidden, for example, to also forbid a comma, a semicolon, and a colon:

```yaml
rules:
  no_description_terminal_punctuation:
    characters: ".,;:!?"
```

As the description is the end of the header, this rule also limits how the header ends.

| Setting      | Description                                       | Default |
|--------------|---------------------------------------------------|---------|
| `enabled`    | Whether the rule is applied.                      | `true`  |
| `severity`   | Whether a violation is an `error` or a `warning`. | `error` |
| `characters` | The characters the description must not end with. | `.!?`   |

## `no_trailing_whitespace`

Forbids spaces, tabs, and other whitespace at the end of any line of the message, in the header,
the body, and the footers. Lines that only contain whitespace are reported as well. Each violation
marks the trailing whitespace.

With Git's default cleanup, trailing whitespace is removed before a commit is created, and CoCoCheck
removes it the same way for a message checked with [`--message-file`](hooks.md#checking-a-message-file).
The rule therefore mainly reports commits in the history, or messages created with
`git commit --cleanup=verbatim`.

| Setting    | Description                                       | Default |
|------------|---------------------------------------------------|---------|
| `enabled`  | Whether the rule is applied.                      | `true`  |
| `severity` | Whether a violation is an `error` or a `warning`. | `error` |

## `required_footers`

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

## `scope_case`

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

## `scope_presence`

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

## `type_case`

Requires the commit type to be lowercase, so that `Feat: Add a feature` is reported.

| Setting    | Description                                       | Default |
|------------|---------------------------------------------------|---------|
| `enabled`  | Whether the rule is applied.                      | `true`  |
| `severity` | Whether a violation is an `error` or a `warning`. | `error` |
