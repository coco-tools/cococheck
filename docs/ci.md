# Using CoCoCheck in CI

In CI, CoCoCheck usually checks the new commits of a pull request or merge request, and the commits
pushed to a branch. To find them, the CI job must fetch the history of the repository, not only the
latest commit, which many CI systems do by default. CoCoCheck fails with an error if a commit of the
range is missing from the clone.

## GitHub Actions

The [CoCoCheck action](https://github.com/coco-tools/cococheck-action) downloads a prebuilt binary
of CoCoCheck and checks the commits of the event that triggered the workflow:

```yaml
on:
  push:
    branches:
      - main
  pull_request:

permissions:
  contents: read

jobs:
  cococheck:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
        with:
          # Fetch the full history, so that the commits to check can be found.
          fetch-depth: 0
      - uses: coco-tools/cococheck-action@v1
```

For a pull request, the action checks the commits between the base and the head of the pull
request. For a push, it checks the pushed commits, or all commits up to the pushed one for the first
push of a new branch. Each release of the action uses a fixed version of CoCoCheck. See the
[README of the action](https://github.com/coco-tools/cococheck-action#readme) for its inputs, like
`config`, `deny-warnings`, `args` for further options like `--format json`, and `version` to choose
another version of CoCoCheck.

Without the action, download CoCoCheck in a step and pass the commits to check with `--from` and
`--to`:

```yaml
    steps:
      - uses: actions/checkout@v7
        with:
          fetch-depth: 0
      - name: Install CoCoCheck
        run: |
          curl -sSfL https://github.com/coco-tools/cococheck/releases/latest/download/cococheck-x86_64-unknown-linux-musl.tar.gz \
            | tar -xz -C "$RUNNER_TEMP" cococheck
          echo "$RUNNER_TEMP" >> "$GITHUB_PATH"
      - name: Check the commits of the pull request
        if: github.event_name == 'pull_request'
        env:
          BASE: ${{ github.event.pull_request.base.sha }}
          HEAD: ${{ github.event.pull_request.head.sha }}
        run: cococheck --color always --from "$BASE" --to "$HEAD"
      - name: Check the pushed commits
        if: github.event_name == 'push'
        env:
          BEFORE: ${{ github.event.before }}
          SHA: ${{ github.sha }}
        run: |
          # For a new branch, BEFORE is all zeros. After a force-push, it is usually not part of the
          # clone. Check only the pushed commit in both cases.
          if git cat-file -e "$BEFORE^{commit}" 2> /dev/null; then
            cococheck --color always --from "$BEFORE" --to "$SHA"
          else
            cococheck --color always "$SHA"
          fi
```

Use the head commit of a pull request, not `github.sha`, which is a merge commit that GitHub creates
for the workflow and that is not part of the pull request.

## GitLab CI

In GitLab CI, set `GIT_DEPTH` to `0` to fetch the full history, and check the commits of a merge
request against the commit that it is based on, `CI_MERGE_REQUEST_DIFF_BASE_SHA`:

```yaml
cococheck:
  image: alpine:3
  variables:
    # Fetch the full history, so that the commits to check can be found.
    GIT_DEPTH: 0
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
  before_script:
    - apk add --no-cache curl
    - curl -sSfL https://github.com/coco-tools/cococheck/releases/latest/download/cococheck-x86_64-unknown-linux-musl.tar.gz
      | tar -xz -C /usr/local/bin cococheck
  script:
    # In merged results pipelines, CI_COMMIT_SHA is a merge commit, so check up to the head of the
    # source branch instead.
    - cococheck --color always --from "$CI_MERGE_REQUEST_DIFF_BASE_SHA"
      --to "${CI_MERGE_REQUEST_SOURCE_BRANCH_SHA:-$CI_COMMIT_SHA}"
```

To check pushed commits in a branch pipeline, use `--from "$CI_COMMIT_BEFORE_SHA"` instead. Like
`github.event.before`, it is all zeros for the first push of a new branch, and may be missing from
the clone after a force-push; handle these cases like in the GitHub Actions example above.

## Pinning the version

The examples download the latest release of CoCoCheck, so a new release with stricter checks can
fail a pipeline that passed before. To avoid this, download a fixed version by replacing
`latest/download` in the URL with `download/v<version>`, for example `download/v0.1.0`.
