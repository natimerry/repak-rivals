# Build

Build from workspace root.

## Requirements

| Requirement | Notes |
| --- | --- |
| Rust | install via <https://rustup.rs/> |
| toolchain | pinned by `rust-toolchain.toml` |
| Oodle DLL/SO | repo includes expected Oodle runtime files |
| KawaiiPhysics binding | managed .NET 8 DLL built by `retoc-rivals` build script |

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

## KawaiiPhysics Binding

`retoc-rivals/build.rs` publishes `UAssetAPI/KawaiiPhysicsBinding` as a managed
`KawaiiPhysicsBinding.dll` with `PublishAot=false`. NativeAOT is intentionally
disabled because UAssetAPI relies on reflection.

The Rust side embeds the published DLL artifacts, extracts them beside the
running binary under `KawaiiPhysicsBinding/`, loads .NET through `hostfxr`, and
calls the managed `PortAsset` export directly. This avoids spawning a helper
process for every asset.

By default the binding is framework-dependent and requires a local .NET
runtime. The runtime config allows major-version roll-forward, so newer .NET
runtimes can satisfy the binding.

Set `RETOC_KAWAII_BINDING_SELF_CONTAINED=true` when building release artifacts
if the binding should publish as a self-contained helper process. This variant
is larger and slower during KawaiiPhysics porting, but it does not require the
user to install .NET separately.

## Workspace Notes

| Path | Purpose |
| --- | --- |
| `repak-gui/` | eframe GUI |
| `retoc-rivals-cli/` | Clap CLI for current workflows |
| `retoc-rivals/` | IoStore conversion/extraction |
| `repak/` | pak parsing/writing |
| `repak_cli/` | older pak-only CLI |
| `oodle_loader/` | Oodle loader glue |
