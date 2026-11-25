# Git hooks

CoCoCheck can check a commit message before the commit is created, in a
[`commit-msg` hook](https://git-scm.com/docs/githooks#_commit_msg), which rejects commits whose
message fails the checks.

## `commit-msg` hook

To set up the hook by hand, create the file `.git/hooks/commit-msg` with the following content and
make it executable with `chmod +x .git/hooks/commit-msg`:

```shell
#!/bin/sh
exec cococheck --message-file "$1"
```

Git runs the hook in the root of the working tree, so CoCoCheck reads the configuration file of the
repository. A message is not checked if it is [ignored](configuration.md#ignored-commits), for
example the message of a merge commit. To create a single commit without running the hook, use
`git commit --no-verify`.

## Hook managers

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
when it is run for the first time, so it requires a [Rust toolchain](installation.md#from-source).

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
[installed](installation.md) first.

## Checking a message file

To check a commit message before the commit is created, pass the file that contains the message
with `--message-file`, or `-` to read the message from standard input. This cannot be combined with
a revision range, `--from`, `--to`, or `--from-latest-tag`.

### Message cleanup

Like Git, CoCoCheck cleans up the message before checking it: It removes comment lines and, for
`git commit --verbose`, the scissors line and the diff below it, and removes surplus whitespace.
Comment lines start with the string configured in Git's `core.commentString` or `core.commentChar`
setting, or `#` if it is not set or set to `auto`. If Git's `commit.cleanup` setting is
`scissors`, `whitespace`, or `verbatim`, CoCoCheck cleans up the message in the same way.

A hook cannot detect a `--cleanup` option passed to `git commit`, or whether the message was
written in an editor. In Git's default cleanup mode, comment lines are only removed if the message
was edited, so a line of a message given with `git commit -m` that starts with the comment string
is part of the commit, but is not checked.
