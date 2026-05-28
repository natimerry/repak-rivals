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
| CLI lacks mapping | pass `--kawaii-physics-usmap mappings.usmap`; if omitted, CLI uses saved GUI config, then downloads the latest mapping |
| only want asset fix | use `retoc-rivals-cli fix-kawaii-physics <dir>` |
| fixing every mod in a mixed download folder | use `retoc-rivals-cli pack-dir "C:\Downloads\Rivals Mods" --output fixed_mods --kawaii-physics` |

Use `--kawaii-physics` when you want fixed installable package files. Use `fix-kawaii-physics <dir>` on an unpacked/raw directory when you want to patch the assets in place and pack them later yourself.

## IoStore Dependencies

When extraction misses imported packages or fails resolving chunks, pass game `Paks`:

```console
retoc-rivals-cli unpack Example.utoc --game-paks-dir "C:\Path\To\MarvelRivals\MarvelGame\Marvel\Content\Paks"
```

For CLI commands that need game containers, omit `--game-paks-dir` only when repak-gui has saved the game `Content\Paks` path. If neither an explicit path nor saved GUI config exists, the CLI stops with an error instead of guessing. Use the base `Content\Paks` directory, not `Content\Paks\~mods`.

If fast-path dependency scan is insufficient:

```console
retoc-rivals-cli unpack Example.utoc --game-paks-dir "C:\Path\To\Paks" --full-iostore-check
```

Fast path opens selected mod containers plus likely dependencies. Full check opens all game `.utoc` files and is slower.

`pack` and `pack-dir` also need game containers when an existing IoStore triple must be transformed with `--obfuscate`, non-default `--compression`, or `--kawaii-physics`. Without those transform flags, IoStore triples are copied as-is.

## Output Naming

| Symptom | Fix |
| --- | --- |
| missing `_9999999_P` | repack without `--no-mod-suffix` |
| duplicate suffix | leave default behavior; tool detects existing suffix |
| existing IoStore copied with old name | pass `--no-mod-suffix` only when intentionally preserving source name |
| want each mod in its own output folder | add `--separate-output-dirs` to `pack` or `pack-dir` |

## Archive Payloads

`.7z`, `.zip`, and `.rar` archives are extracted to temp and scanned. If an archive contains multiple mods, GUI may create multiple install rows and CLI may perform multiple package operations.

For a mixed folder containing loose IoStore triples, legacy paks, raw mod folders, and archives, use `retoc-rivals-cli pack-dir`. It scans the folder first, logs each archive extraction, and batches IoStore extraction for transform flows so game containers are opened once for the input set instead of once per archive.

## Legacy Pak vs IoStore

| Files present | Classification |
| --- | --- |
| `Mod.pak` only | legacy pak, handled by `repak` |
| `Mod.pak`, `Mod.utoc`, `Mod.ucas` | IoStore, handled by `retoc-rivals` |
| `.utoc` or `.ucas` missing companions | invalid IoStore input |

## Build Warnings

Known workspace warnings include old dead code, private interface visibility, and dependency naming warnings. `cargo check -p retoc-rivals-cli` passing is the main validation for CLI changes.
