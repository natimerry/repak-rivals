# Troubleshooting

## Game Path

| Symptom | Fix |
| --- | --- |
| GUI cannot find game | click `Browse`, select `MarvelRivals\MarvelGame\Marvel\Content\Paks\~mods` |
| `Launch Game` disabled | install still works; launch from Steam manually |
| wrong mod folder | choose actual `~mods`, not parent `Paks`, for GUI mod management |

## Locked Files

Close Marvel Rivals before installing, deleting, enabling, or disabling mods. Windows can block renames/copies while package files are open.

## KawaiiPhysics

| Symptom | Fix |
| --- | --- |
| GUI blocks install | set `.usmap` with `File -> Select Mapping file` or disable `Kawaii porter` |
| CLI lacks mapping | pass `--kawaii-physics-usmap mappings.usmap`; if omitted, CLI tries auto-download |
| only want asset fix | use `retoc-rivals-cli pack <dir> --kawaii-physics-only` |

## IoStore Dependencies

When extraction misses imported packages or fails resolving chunks, pass game `Paks`:

```console
retoc-rivals-cli unpack Example.utoc --game-paks-dir "C:\Path\To\MarvelRivals\MarvelGame\Marvel\Content\Paks"
```

If fast-path dependency scan is insufficient:

```console
retoc-rivals-cli unpack Example.utoc --game-paks-dir "C:\Path\To\Paks" --full-iostore-check
```

Fast path opens selected mod containers plus likely dependencies. Full check opens all game `.utoc` files and is slower.

## Output Naming

| Symptom | Fix |
| --- | --- |
| missing `_9999999_P` | repack without `--no-mod-suffix` |
| duplicate suffix | leave default behavior; tool detects existing suffix |
| existing IoStore copied with old name | pass `--no-mod-suffix` only when intentionally preserving source name |

## Archive Payloads

Archives are extracted to temp and scanned. If archive contains multiple mods, GUI may create multiple install rows and CLI may perform multiple package operations.

## Legacy Pak vs IoStore

| Files present | Classification |
| --- | --- |
| `Mod.pak` only | legacy pak, handled by `repak` |
| `Mod.pak`, `Mod.utoc`, `Mod.ucas` | IoStore, handled by `retoc-rivals` |
| `.utoc` or `.ucas` missing companions | invalid IoStore input |

## Build Warnings

Known workspace warnings include old dead code, private interface visibility, and dependency naming warnings. `cargo check -p retoc-rivals-cli` passing is the main validation for CLI changes.
