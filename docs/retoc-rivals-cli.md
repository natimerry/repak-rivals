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

KawaiiPhysics:

```console
retoc-rivals-cli pack path\to\MyModFolder --kawaii-physics --kawaii-physics-usmap mappings.usmap
retoc-rivals-cli fix-kawaii-physics path\to\MyModFolder --usmap mappings.usmap
```

KawaiiPhysics modes:

| Mode | Input | Output | Use when |
| --- | --- | --- | --- |
| `--kawaii-physics` | raw dir, legacy pak/archive, or IoStore input | writes repacked `.pak/.utoc/.ucas` output | you want fixed installable mod files |
| `fix-kawaii-physics <dir>` | unpacked/raw asset directory only | modifies that directory in place; no package output | you want to patch assets before packing later |

If `--kawaii-physics-usmap` is omitted, the CLI uses saved GUI config first, then downloads the latest Rivals depot mapping. IoStore inputs repacked with `--kawaii-physics` also need the game `Content\Paks` directory; pass `--game-paks-dir` or open repak-gui once so its saved config can be reused.

Compression:

```console
retoc-rivals-cli pack path\to\MyModFolder --compression oodle
retoc-rivals-cli pack path\to\MyModFolder --compression zstd
retoc-rivals-cli pack path\to\MyModFolder --compression none
```

| Option | Default | Meaning |
| --- | --- | --- |
| `-o, --output <DIR>` | input parent | output directory |
| `--mount-point <MOUNT>` | `../../../` | generated fake pak mount point |
| `--path-hash-seed <SEED>` | `00000000` | generated fake pak path hash seed |
| `--no-mod-suffix` | off | do not append `_9999999_P` |
| `--obfuscate` | off | obfuscate generated IoStore |
| `--compression <METHOD>` | `oodle` | `none`, `zlib`, `zstd`, `lz4`, `oodle` |
| `--kawaii-physics` | off | port assets while converting |
| `--kawaii-physics-usmap <PATH>` | saved GUI config, then auto-download | mapping file |
| `--game-paks-dir <DIR>` | saved GUI config if available | deps for repacking IoStore with KawaiiPhysics |
| `--full-iostore-check` | off | slow all-container dependency path |

Output rules:

| Behavior | Rule |
| --- | --- |
| suffix | appends `_9999999_P` unless `--no-mod-suffix` |
| generated files | `.pak`, `.utoc`, `.ucas` |
| generated `.pak` | fake pak containing `chunknames` |
| IoStore input without `--kawaii-physics` | copied/installed, not converted |
| IoStore input with `--kawaii-physics` | to-legacy temp extraction, Kawaii port, re-pack |

`pack <dir>` treats a raw asset directory as one mod. Use `pack-dir` for a drop folder that contains a mix of raw mod folders, loose IoStore triples, legacy paks, and archives.

## `pack-dir`

Use `pack-dir` for a mixed download folder.

```console
retoc-rivals-cli pack-dir "C:\Downloads\Rivals Mods" --output "C:\Path\To\~mods"
retoc-rivals-cli pack-dir "C:\Downloads\Rivals Mods" --output fixed_mods --kawaii-physics
retoc-rivals-cli pack-dir "C:\Users\soham\Desktop\mods\WhitePimpStuff" --output "C:\Users\soham\Desktop\whitepfixed" --kawaii-physics --game-paks-dir "D:\SteamLibrary\steamapps\common\MarvelRivals\MarvelGame\Marvel\Content\Paks" --kawaii-physics-usmap mappings.usmap
```

`pack-dir` discovers direct raw cooked mod folders, recursive IoStore triples, legacy paks, and `.7z`/`.zip`/`.rar` archives. It logs tree scanning and archive extraction. When KawaiiPhysics is enabled for IoStore inputs, it batches to-legacy extraction so game containers are opened once for the input set instead of once per archive.

`pack-dir` supports `--kawaii-physics` because it is meant to produce fixed package outputs for every discovered mod. For in-place asset-only patching, unpack one mod to a raw directory first, then use `fix-kawaii-physics <dir>`.

| Option | Default | Meaning |
| --- | --- | --- |
| `-o, --output <DIR>` | input parent | output directory for every packed mod |
| `--mount-point <MOUNT>` | `../../../` | generated fake pak mount point |
| `--path-hash-seed <SEED>` | `00000000` | generated fake pak path hash seed |
| `--no-mod-suffix` | off | do not append `_9999999_P` |
| `--obfuscate` | off | obfuscate generated IoStore |
| `--compression <METHOD>` | `oodle` | `none`, `zlib`, `zstd`, `lz4`, `oodle` |
| `--kawaii-physics` | off | port assets while converting |
| `--kawaii-physics-usmap <PATH>` | saved GUI config, then auto-download | mapping file |
| `--game-paks-dir <DIR>` | saved GUI config if available | deps for repacking IoStore with KawaiiPhysics |
| `--full-iostore-check` | off | open all game IoStore containers |

## `fix-kawaii-physics`

```console
retoc-rivals-cli fix-kawaii-physics
retoc-rivals-cli fix-kawaii-physics unpacked-mod
retoc-rivals-cli fix-kawaii-physics --output fixed-mods
retoc-rivals-cli fix-kawaii-physics --usmap mappings.usmap
```

| Option | Default | Meaning |
| --- | --- | --- |
| `<INPUT>` | none | optional raw asset directory to patch in-place |
| `-o, --output <DIR>` | `fixed-mods` | rebuilt mod output directory |
| `-u, --usmap <PATH>` | saved config, then auto-download | mapping file |

With `<INPUT>`, this command walks the directory for `.uasset` files and patches KawaiiPhysics assets in place. Without `<INPUT>`, it uses saved GUI config for installed mods dir and game `Paks` dir, rebuilds installed IoStore mods, and writes fixed packages to `--output`.

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
