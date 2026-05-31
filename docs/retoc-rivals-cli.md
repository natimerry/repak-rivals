# retoc-rivals-cli

`retoc-rivals-cli` is scriptable package tooling for Marvel Rivals. It handles modern IoStore triples, legacy paks, archives, raw directories, mixed mod folders, manifests, filters, compression, and KawaiiPhysics workflows.

## Run

```console
cargo run -p retoc-rivals-cli -- --help
cargo run -p retoc-rivals-cli -- <command> --help
retoc-rivals-cli --help
```

## Global Options

| Option | Default | Meaning |
| --- | --- | --- |
| `-a, --aes-key <AES_KEY>` | Marvel Rivals AES key | IoStore AES key |
| `-v, --verbose` | off | tracing verbosity |
| `-h, --help` | | help |
| `-V, --version` | | version |

## Commands

| Command | Alias | Purpose |
| --- | --- | --- |
| `info` | | show package type/info |
| `manifest` | | emit IoStore manifest JSON or extraction filters |
| `unpack` | `extract` | extract one or more packages/archives/directories |
| `unpack-dir` | `extract-dir` | recursively batch-extract package files below a directory |
| `pack` | | package raw dirs, repack legacy pak/archive, install/copy IoStore triples |
| `pack-dir` | | package every mod found below a mixed directory |
| `fix-kawaii-physics` | | patch a raw asset directory in-place or rebuild installed IoStore mods using saved GUI config |

## Input Classification

| Input | Detection | Backend |
| --- | --- | --- |
| `.pak` only | no same-stem `.utoc/.ucas` | `repak` legacy pak |
| `.pak/.utoc/.ucas` triple | any matched extension resolves companions | `retoc-rivals` IoStore |
| directory with package files | recursive package scan | batch IoStore + legacy pak handling |
| directory with raw assets | raw folder, especially `.uasset` content | IoStore packaging |
| mixed directory | direct raw mod dirs plus recursive package/archive scan | `pack-dir` |
| `.7z` / `.zip` / `.rar` | extract to temp then classify payload | same as payload |

Keep IoStore companions together:

```text
ExampleMod_9999999_P.pak
ExampleMod_9999999_P.utoc
ExampleMod_9999999_P.ucas
```

## `info`

```console
retoc-rivals-cli info ExampleMod_9999999_P.utoc
retoc-rivals-cli info ExampleMod_9999999_P.pak
retoc-rivals-cli info "C:\Downloads\SomeArchive.rar"
retoc-rivals-cli info "C:\Path\To\ModDirectory"
```

| Input kind | Output |
| --- | --- |
| IoStore | companion paths + retoc container info |
| legacy pak | version, mount point, encrypted index, encryption GUID, path hash seed, file count |
| directory | package counts and paths, or raw-directory marker |
| archive | archive path + classified payload info |

## `manifest`

```console
retoc-rivals-cli manifest ExampleMod_9999999_P.utoc
retoc-rivals-cli manifest ExampleMod_9999999_P.utoc --output manifest.json
retoc-rivals-cli manifest ExampleMod_9999999_P.utoc --filters
```

| Option | Meaning |
| --- | --- |
| `-o, --output <PATH>` | write manifest JSON to file |
| `--filters` | print retoc to-legacy filter paths instead of JSON |

Use `--filters` before selective `unpack --filter ...` work.

## `unpack`

```console
retoc-rivals-cli unpack ExampleMod_9999999_P.utoc
retoc-rivals-cli unpack ExampleMod_9999999_P.utoc --output unpacked-example
retoc-rivals-cli unpack First.utoc Second.utoc Third.pak
retoc-rivals-cli unpack SomeArchive.zip --output unpacked-archive
```

IoStore filters:

```console
retoc-rivals-cli unpack ExampleMod_9999999_P.utoc --filter Characters --filter UI
```

Dependency containers:

```console
retoc-rivals-cli unpack ExampleMod_9999999_P.utoc --game-paks-dir "C:\Path\To\MarvelRivals\MarvelGame\Marvel\Content\Paks"
retoc-rivals-cli unpack ExampleMod_9999999_P.utoc --game-paks-dir "C:\Path\To\Paks" --full-iostore-check
```

| Option | Meaning |
| --- | --- |
| `-o, --output <DIR>` | output directory; one input only |
| `-v, --verbose` | verbose retoc output |
| `-f, --filter <FILTER>` | repeatable IoStore asset/package filter |
| `--game-paks-dir <DIR>` | game `Paks` dir for dependency containers |
| `--full-iostore-check` | open all game IoStore containers instead of fast-path set |

Fast path opens selected mod containers plus likely dependency containers (`global`, character chunks). Full check opens all game `.utoc` files and is slower.

## `unpack-dir`

```console
retoc-rivals-cli unpack-dir "C:\Downloads\Rivals Mods"
retoc-rivals-cli unpack-dir "C:\Downloads\Rivals Mods" --output-prefix extracted_
retoc-rivals-cli unpack-dir "C:\Downloads\Rivals Mods" --filter Characters
```

| Option | Default | Meaning |
| --- | --- | --- |
| `--output-prefix <PREFIX>` | `unpacked_` | generated output directory prefix |
| `-v, --verbose` | off | verbose retoc output |
| `-f, --filter <FILTER>` | none | repeatable IoStore filter applied to every item |
| `--game-paks-dir <DIR>` | saved GUI config if available | dependency container dir |
| `--full-iostore-check` | off | slow all-container dependency path |

`unpack-dir` uses retoc batch extraction for IoStore packages.

## `pack`

Raw folder to IoStore:

```console
retoc-rivals-cli pack path\to\MyModFolder
retoc-rivals-cli pack path\to\MyModFolder --output "C:\Path\To\~mods"
retoc-rivals-cli pack path\to\MyModFolder --output fixed_mods --separate-output-dirs
```

Legacy pak/archive to IoStore:

```console
retoc-rivals-cli pack OldMod.pak --output "C:\Path\To\~mods"
retoc-rivals-cli pack DownloadedMod.rar --output "C:\Path\To\~mods"
```

Existing IoStore triple install/copy:

```console
retoc-rivals-cli pack ExistingMod_9999999_P.utoc --output "C:\Path\To\~mods"
```

Repack an existing IoStore triple with obfuscation:

```console
retoc-rivals-cli pack ExistingMod_9999999_P.utoc --output fixed_mods --game-paks-dir "C:\Path\To\Paks" --full-iostore-check --obfuscate
```

KawaiiPhysics and hidden materials:

```console
retoc-rivals-cli pack path\to\MyModFolder --kawaii-physics --kawaii-physics-usmap mappings.usmap
retoc-rivals-cli fix-kawaii-physics path\to\MyModFolder --usmap mappings.usmap
retoc-rivals-cli pack path\to\MyModFolder --patch-default-hidden-mats --kawaii-physics-usmap mappings.usmap
retoc-rivals-cli pack path\to\MyModFolder --default-hidden-material-bitmaps 0x0FFF0000,0x0FFF0000,0x0EFB0000 --kawaii-physics-usmap mappings.usmap
```

KawaiiPhysics modes:

| Mode | Input | Output | Use when |
| --- | --- | --- | --- |
| `--kawaii-physics` | raw dir, legacy pak/archive, or IoStore input | writes repacked `.pak/.utoc/.ucas` output | you want KawaiiPhysics assets ported while packing |
| `--patch-default-hidden-mats` | raw dir, legacy pak/archive, or IoStore input | writes repacked `.pak/.utoc/.ucas` output | you want `DefaultHiddenMaterials` from carrier data |
| `--default-hidden-material-bitmaps <MASKS>` | raw dir, legacy pak/archive, or IoStore input | writes repacked `.pak/.utoc/.ucas` output | you want explicit hidden-material masks |
| `fix-kawaii-physics <dir>` | unpacked/raw asset directory only | modifies that directory in place; no package output | you want to patch assets before packing later |

`--kawaii-physics` is separate from hidden-material patching. Use `--patch-default-hidden-mats` or `--default-hidden-material-bitmaps` when you also want `LODInfo.DefaultHiddenMaterials`.

If `--kawaii-physics-usmap` is omitted, the CLI uses saved GUI config first, then downloads the latest Rivals depot mapping. IoStore inputs repacked with `--kawaii-physics`, `--patch-default-hidden-mats`, or `--default-hidden-material-bitmaps` also need the game `Content\Paks` directory; pass `--game-paks-dir` or open repak-gui once so its saved config can be reused.

DefaultHiddenMaterials masks:

```console
retoc-rivals-cli pack path\to\MyModFolder --default-hidden-material-bitmaps 0x0FFF0000
retoc-rivals-cli pack path\to\MyModFolder --default-hidden-material-bitmaps 0x0FFF0000,0x0FFF0000,0x0EFB0000
retoc-rivals-cli pack path\to\MyModFolder --default-hidden-material-bitmaps 268369920,268369920,251330560
```

| Rule | Meaning |
| --- | --- |
| one mask per LOD | first mask is LOD0, second is LOD1, third is LOD2, etc. |
| one mask total | reused for every LOD |
| fewer masks than LODs | last mask is reused for remaining LODs |
| bit index | bit `0` controls material slot `0`, bit `1` controls slot `1`, etc. |
| bit value | `1` writes `true`/hidden by default; `0` writes `false`/visible by default |
| accepted forms | CLI accepts comma-separated decimal or `0x` hex `u64` values |
| max slot bit | masks are `u64`, so they can directly set slots `0` through `63` |
| array length | uses the mesh material count when available; otherwise uses the highest set bit plus one |

The built-in default preset used by the GUI `Default` mode is:

```text
0x0FFF0000,0x0FFF0000,0x0EFB0000
```

That writes LOD0=`0x0FFF0000`, LOD1=`0x0FFF0000`, and LOD2=`0x0EFB0000`. In custom workflows, build masks from the material slot indices you want hidden.

Compression:

```console
retoc-rivals-cli pack path\to\MyModFolder --compression oodle
retoc-rivals-cli pack path\to\MyModFolder --compression zstd
retoc-rivals-cli pack path\to\MyModFolder --compression none
```

| Option | Default | Meaning |
| --- | --- | --- |
| `-o, --output <DIR>` | input parent | output directory |
| `--separate-output-dirs` | off | write each packed mod under `--output\<input-name>` |
| `--mount-point <MOUNT>` | `../../../` | generated fake pak mount point |
| `--path-hash-seed <SEED>` | `00000000` | generated fake pak path hash seed |
| `--no-mod-suffix` | off | do not append `_9999999_P` |
| `--obfuscate` | off | obfuscate generated IoStore |
| `--compression <METHOD>` | `oodle` | `none`, `zlib`, `zstd`, `lz4`, `oodle` |
| `--kawaii-physics` | off | port KawaiiPhysics assets while converting |
| `--kawaii-physics-usmap <PATH>` | saved GUI config, then auto-download | mapping file |
| `--patch-default-hidden-mats` | off | patch `LODInfo.DefaultHiddenMaterials` from carrier data |
| `--default-hidden-material-bitmaps <MASKS>` | off | override `LODInfo.DefaultHiddenMaterials` with comma-separated per-LOD integer bitmaps |
| `--game-paks-dir <DIR>` | saved GUI config if available | deps for repacking IoStore with KawaiiPhysics, obfuscation, or non-default compression |
| `--full-iostore-check` | off | slow all-container dependency path |

Output rules:

| Behavior | Rule |
| --- | --- |
| suffix | appends `_9999999_P` unless `--no-mod-suffix` |
| generated files | `.pak`, `.utoc`, `.ucas` |
| generated `.pak` | fake pak containing `chunknames` |
| IoStore input without transform flags | copied/installed, not converted |
| IoStore input with `--obfuscate` | to-legacy temp extraction, re-pack with obfuscated IoStore |
| IoStore input with non-default `--compression` | to-legacy temp extraction, re-pack with selected compression |
| IoStore input with `--kawaii-physics` | to-legacy temp extraction, Kawaii port, re-pack |
| IoStore input with `--patch-default-hidden-mats` | to-legacy temp extraction, carrier autodetect, re-pack |
| IoStore input with `--default-hidden-material-bitmaps` | to-legacy temp extraction, mask override, re-pack |

`pack <dir>` treats a raw asset directory as one mod. Use `pack-dir` for a drop folder that contains a mix of raw mod folders, loose IoStore triples, legacy paks, and archives.

## `pack-dir`

Use `pack-dir` for a mixed download folder.

```console
retoc-rivals-cli pack-dir "C:\Downloads\Rivals Mods" --output "C:\Path\To\~mods"
retoc-rivals-cli pack-dir "C:\Downloads\Rivals Mods" --output fixed_mods --separate-output-dirs
retoc-rivals-cli pack-dir "C:\Downloads\Rivals Mods" --output fixed_mods --kawaii-physics
retoc-rivals-cli pack-dir "C:\Users\soham\Desktop\mods\WhitePimpStuff" --output "C:\Users\soham\Desktop\whitepfixed" --kawaii-physics --game-paks-dir "D:\SteamLibrary\steamapps\common\MarvelRivals\MarvelGame\Marvel\Content\Paks" --kawaii-physics-usmap mappings.usmap
```

`pack-dir` discovers direct raw cooked mod folders, recursive IoStore triples, legacy paks, and `.7z`/`.zip`/`.rar` archives. It logs tree scanning and archive extraction. When IoStore inputs need repacking (`--obfuscate`, non-default `--compression`, `--kawaii-physics`, `--patch-default-hidden-mats`, or `--default-hidden-material-bitmaps`), it batches to-legacy extraction so game containers are opened once for the input set instead of once per archive.

`pack-dir` supports `--kawaii-physics` because it is meant to produce fixed package outputs for every discovered mod. For in-place asset-only patching, unpack one mod to a raw directory first, then use `fix-kawaii-physics <dir>`.

| Option | Default | Meaning |
| --- | --- | --- |
| `-o, --output <DIR>` | input parent | output directory for every packed mod |
| `--separate-output-dirs` | off | write each packed mod under `--output\<mod-name>` |
| `--mount-point <MOUNT>` | `../../../` | generated fake pak mount point |
| `--path-hash-seed <SEED>` | `00000000` | generated fake pak path hash seed |
| `--no-mod-suffix` | off | do not append `_9999999_P` |
| `--obfuscate` | off | obfuscate generated IoStore |
| `--compression <METHOD>` | `oodle` | `none`, `zlib`, `zstd`, `lz4`, `oodle` |
| `--kawaii-physics` | off | port KawaiiPhysics assets while converting |
| `--kawaii-physics-usmap <PATH>` | saved GUI config, then auto-download | mapping file |
| `--patch-default-hidden-mats` | off | patch `LODInfo.DefaultHiddenMaterials` from carrier data |
| `--default-hidden-material-bitmaps <MASKS>` | off | override `LODInfo.DefaultHiddenMaterials` with comma-separated per-LOD integer bitmaps |
| `--game-paks-dir <DIR>` | saved GUI config if available | deps for repacking IoStore with KawaiiPhysics, obfuscation, or non-default compression |
| `--full-iostore-check` | off | open all game IoStore containers |

## `fix-kawaii-physics`

```console
retoc-rivals-cli fix-kawaii-physics
retoc-rivals-cli fix-kawaii-physics unpacked-mod
retoc-rivals-cli fix-kawaii-physics --output fixed-mods
retoc-rivals-cli fix-kawaii-physics --usmap mappings.usmap
retoc-rivals-cli fix-kawaii-physics unpacked-mod --patch-default-hidden-mats
retoc-rivals-cli fix-kawaii-physics unpacked-mod --default-hidden-material-bitmaps 0x0FFF0000,0x0FFF0000,0x0EFB0000
```

| Option | Default | Meaning |
| --- | --- | --- |
| `<INPUT>` | none | optional raw asset directory to patch in-place |
| `-o, --output <DIR>` | `fixed-mods` | rebuilt mod output directory |
| `-u, --usmap <PATH>` | saved config, then auto-download | mapping file |
| `--patch-default-hidden-mats` | off | patch `LODInfo.DefaultHiddenMaterials` from carrier data |
| `--default-hidden-material-bitmaps <MASKS>` | off | override `LODInfo.DefaultHiddenMaterials` with comma-separated per-LOD integer bitmaps |

With `<INPUT>`, this command walks the directory for `.uasset` files and patches KawaiiPhysics assets in place. Add `--patch-default-hidden-mats` or `--default-hidden-material-bitmaps` to also patch `DefaultHiddenMaterials`. Without `<INPUT>`, it uses saved GUI config for installed mods dir and game `Paks` dir, rebuilds installed IoStore mods, and writes fixed packages to `--output`.

## Recipes

| Task | Command |
| --- | --- |
| inspect archive | `retoc-rivals-cli info SomeMod.zip` |
| generate filters | `retoc-rivals-cli manifest SomeMod_9999999_P.utoc --filters` |
| extract IoStore | `retoc-rivals-cli unpack SomeMod_9999999_P.utoc --output unpacked` |
| batch extract downloads | `retoc-rivals-cli unpack-dir "C:\Downloads\Rivals Mods"` |
| rebuild edited folder | `retoc-rivals-cli pack unpacked --output "C:\Path\To\~mods"` |
| pack mixed download folder | `retoc-rivals-cli pack-dir "C:\Downloads\Rivals Mods" --output "C:\Path\To\~mods"` |
| fix every mod in a mixed folder | `retoc-rivals-cli pack-dir "C:\Downloads\Rivals Mods" --output fixed_mods --kawaii-physics` |
| in-place Kawaii fix | `retoc-rivals-cli fix-kawaii-physics unpacked` |
