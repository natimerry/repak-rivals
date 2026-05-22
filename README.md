# repak-rivals

Marvel Rivals mod packaging, conversion, extraction, inspection, and install toolset.

## Use The Right Tool

| Tool | Use for | Avoid for |
| --- | --- | --- |
| `repak-gui` | normal mod install/manage workflow; archives; raw folders; legacy pak repack; IoStore mods | script automation |
| `retoc-rivals-cli` | scriptable `.pak`/`.utoc`/`.ucas`, archive, directory, manifest, KawaiiPhysics, and conversion workflows | visual mod management |
| `repak_cli` | pak-only legacy inspection/extraction/packing | modern Rivals IoStore output (`.utoc`/`.ucas`) |

Modern Marvel Rivals mods normally install into:

```text
MarvelRivals\MarvelGame\Marvel\Content\Paks\~mods
```

## Quick Start

1. Download latest release from Nexus Mods: <https://www.nexusmods.com/marvelrivals/mods/1717>.
2. Open `repak-gui`.
3. Confirm or browse to `Content\Paks\~mods`.
4. Drag in `.zip`, `.rar`, `.pak`, IoStore triples, or folders.
5. Review install rows/options.
6. Click `Install mod`.
7. Launch game from GUI or Steam.

## Wiki

| Page | Contents |
| --- | --- |
| [GUI](docs/gui.md) | install flow, mod list, tags, file table, KawaiiPhysics mapping |
| [retoc-rivals-cli](docs/retoc-rivals-cli.md) | commands, accepted inputs, examples, filters, compression, full IoStore checks |
| [Build](docs/build.md) | Rust builds, run commands, workspace packages |
| [Troubleshooting](docs/troubleshooting.md) | game path, locked files, KawaiiPhysics, IoStore dependency failures |
| [Release Automation](docs/release-automation.md) | Nexus Mods workflow inputs and config |
| [Changelog](CHANGELOG.md) | release history |

## Workspace

| Path | Purpose |
| --- | --- |
| `repak-gui/` | primary GUI app |
| `retoc-rivals-cli/` | current scriptable Rivals CLI |
| `retoc-rivals/` | IoStore conversion/extraction library |
| `repak/` | pak reader/writer library |
| `repak_cli/` | older pak-only CLI |
| `uasset-mesh-patch-rivals/` | mesh patch helper |
| `usmap/` | mapping-related workspace content |

## Common Commands

```console
cargo run -p repak-gui
cargo run -p retoc-rivals-cli -- --help
cargo build -p repak-gui --release
cargo build -p retoc-rivals-cli --release
cargo check
```

## CLI Shortcuts

```console
retoc-rivals-cli info ExampleMod_9999999_P.utoc
retoc-rivals-cli manifest ExampleMod_9999999_P.utoc --filters
retoc-rivals-cli unpack ExampleMod_9999999_P.utoc --output unpacked
retoc-rivals-cli unpack-dir "C:\Downloads\Rivals Mods"
retoc-rivals-cli pack unpacked --output "C:\Path\To\~mods" --compression oodle
retoc-rivals-cli pack unpacked --kawaii-physics-only
retoc-rivals-cli fix-kawaii-physics --output fixed-mods
```

See [docs/retoc-rivals-cli.md](docs/retoc-rivals-cli.md) for full command behavior.
