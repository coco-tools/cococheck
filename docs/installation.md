# Installation

## Prebuilt binaries

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

## From source

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
