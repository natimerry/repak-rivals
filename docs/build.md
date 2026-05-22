# Build

Build from workspace root.

## Requirements

| Requirement | Notes |
| --- | --- |
| Rust | install via <https://rustup.rs/> |
| toolchain | pinned by `rust-toolchain.toml` |
| Oodle DLL/SO | repo includes expected Oodle runtime files |
| KawaiiPhysics helper | built by `retoc-rivals` build script |

## Packages

| Package | Command |
| --- | --- |
| GUI | `cargo build -p repak-gui --release` |
| current CLI | `cargo build -p retoc-rivals-cli --release` |
| old pak CLI | `cargo build -p repak_cli --release` |
| retoc library | `cargo check -p retoc` |

## Run From Source

```console
cargo run -p repak-gui
cargo run -p retoc-rivals-cli -- --help
cargo run -p retoc-rivals-cli -- pack --help
```

## Checks

```console
cargo fmt --check
cargo check
cargo test
```

For a narrower CLI loop:

```console
cargo fmt -p retoc -p retoc-rivals-cli
cargo check -p retoc-rivals-cli
```

## Release Builds

```console
cargo build -p repak-gui --release
cargo build -p retoc-rivals-cli --release
```

Release binaries land under:

```text
target\release\
```

## Workspace Notes

| Path | Purpose |
| --- | --- |
| `repak-gui/` | eframe GUI |
| `retoc-rivals-cli/` | Clap CLI for current workflows |
| `retoc-rivals/` | IoStore conversion/extraction |
| `repak/` | pak parsing/writing |
| `repak_cli/` | older pak-only CLI |
| `oodle_loader/` | Oodle loader glue |
