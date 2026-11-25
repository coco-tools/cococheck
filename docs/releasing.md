# Releasing

This document describes how CoCoCheck is released. It is meant for maintainers; users only need
the [releases](https://github.com/coco-tools/cococheck/releases) and the [README](../README.md).

Releases are automated with [release-plz](https://release-plz.dev/), configured in
[`release-plz.toml`](../release-plz.toml), and the
[release workflow](../.github/workflows/release-plz.yml). A release consists of:

- the `cococheck` and `cococheck-core` packages on [crates.io](https://crates.io/crates/cococheck),
  which always have the same version,
- a Git tag `v<version>`,
- a [GitHub release](https://github.com/coco-tools/cococheck/releases) with the changes of the
  release and the [prebuilt binaries](installation.md#prebuilt-binaries),
- a new section in the [`CHANGELOG.md`](../CHANGELOG.md).

## Release pull request

On every push to `main`, release-plz creates or updates a pull request titled
`chore: Release v<version>`. It bumps the version in `Cargo.toml` and `Cargo.lock`, and adds the
changes since the last release to the changelog. The version is derived from the commits following
[Semantic Versioning](https://semver.org/): before 1.0, a breaking change bumps the minor version,
and all other changes bump the patch version.

Only the commits that matter to users of CoCoCheck lead to a release pull request, and only they
are added to the changelog:

| Commits                                                      | Changelog section      |
|--------------------------------------------------------------|------------------------|
| `feat`                                                       | Added                  |
| `fix`                                                        | Fixed                  |
| `perf`                                                       | Performance            |
| `deps`, the updates of the Rust crates by Renovate           | Updated dependencies   |
| `revert`, and breaking changes of any other type             | Changed                |
| `build`, `chore`, `ci`, `docs`, `refactor`, `style`, `test`  | Not listed             |

A breaking change is marked with a `!` after the type or scope, or with a `BREAKING CHANGE` footer,
and is always listed, whatever its type. Renovate uses the `ci` type for the updates of GitHub
Actions, so that they do not lead to a release. Choose the type of a commit with this in mind: a
change that users notice should not be hidden in a `chore` or `refactor` commit.

## Releasing

1. Review the release pull request, especially the version and the changelog. If needed, edit the
   changelog in the pull request right before merging it, for example to describe a breaking change
   in more detail.
2. Merge the release pull request. Its commit has no `Signed-off-by` footer, so it is ignored by the
   CoCoCheck check with a pattern in [`cococheck.yml`](../cococheck.yml), which must be kept in sync
   with `pr_name` in `release-plz.toml`.
3. The release workflow runs for the push to `main`:
   1. The `release` job publishes both packages to crates.io, and creates the Git tag and a draft
      of the GitHub release.
   2. The `binaries` job builds the binaries for all platforms with the
      [binaries workflow](../.github/workflows/binaries.yml), and uploads them with their
      checksums to the draft.
   3. The `publish` job publishes the GitHub release and marks it as the latest release.
4. Update the [CoCoCheck action](https://github.com/coco-tools/cococheck-action) to the new
   version. Renovate proposes the update in the action repository as soon as the release is
   published. Merging it creates a new release of the action, which then also checks the commits of
   this repository with the new version.

The GitHub release is created as a draft first, because the repository uses immutable releases,
which do not allow adding files to a published release.

## Fixing a failed release

- **The packages were not published.** Fix the cause, for example in the workflow, and run the
  failed jobs of the release workflow again. release-plz skips packages that are already published.
- **A binary could not be built or uploaded.** The release remains a draft. Run the failed jobs of
  the release workflow again. If the binaries workflow itself has to be fixed, merge the fix, and
  then start the binaries workflow manually from the Actions tab with the tag of the release, for
  example `v0.2.0`. This builds the binaries from the tag with the fixed workflow and uploads them
  to the draft. Then publish the draft on the release page.

A version that is published on crates.io cannot be replaced, and a published GitHub release is
immutable. To correct a broken release, release a new patch version.

## Setup

The release workflow needs the following setup, which is already in place:

- **Trusted publishing on crates.io** for both packages, with the repository
  `coco-tools/cococheck` and the workflow `release-plz.yml`. The workflow requests a short-lived
  token for crates.io with OpenID Connect, so no crates.io token is stored in the repository. See
  the [crates.io documentation](https://crates.io/docs/trusted-publishing).
- **The `RELEASE_PLZ_TOKEN` secret**, a personal access token with write access to the contents and
  the pull requests of the repository. release-plz uses it instead of the token of the workflow,
  because GitHub does not start workflows for pull requests and tags created with the latter, so
  the CI would not run for the release pull request.
- **Immutable releases**, enabled in the settings of the repository.
