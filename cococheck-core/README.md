# cococheck-core

The commit message parser, the configuration, and the rules of
[CoCoCheck](https://github.com/coco-tools/cococheck), a checker for
[Conventional Commits](https://www.conventionalcommits.org/) messages.

> [!WARNING]
> This crate is an internal part of CoCoCheck. It is only published because the `cococheck`
> command-line tool depends on it. Its API has no stability guarantees and can change in
> incompatible ways in any release.

To check commit messages, install the [`cococheck`](https://crates.io/crates/cococheck)
command-line tool instead:

```shell
cargo install cococheck
```

See the [CoCoCheck README](https://github.com/coco-tools/cococheck#readme) for the usage, and the
[configuration](https://github.com/coco-tools/cococheck/blob/main/docs/configuration.md) and
[rule](https://github.com/coco-tools/cococheck/blob/main/docs/rules.md) reference.

## License

Licensed under the [Apache License, Version 2.0](https://github.com/coco-tools/cococheck/blob/main/LICENSE).
