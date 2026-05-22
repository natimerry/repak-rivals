# retoc-rivals-cli

`retoc-rivals-cli` is scriptable package tooling for Marvel Rivals. It handles modern IoStore triples, legacy paks, archives, raw directories, manifests, filters, compression, and KawaiiPhysics workflows.

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
| `fix-kawaii-physics` | | rebuild installed IoStore mods using saved GUI config |

## Input Classification

| Input | Detection | Backend |
| --- | --- | --- |
| `.pak` only | no same-stem `.utoc/.ucas` | `repak` legacy pak |
| `.pak/.utoc/.ucas` triple | any matched extension resolves companions | `retoc-rivals` IoStore |
| directory with package files | recursive package scan | batch IoStore + legacy pak handling |
| directory with raw assets | raw folder, especially `.uasset` content | IoStore packaging |
| `.zip` / `.rar` | extract to temp then classify payload | same as payload |

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
retoc-rivals-cli pack path\to\MyModFolder --kawaii-physics-only --kawaii-physics-usmap mappings.usmap
```

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
| `--kawaii-physics-only` | off | port directory in-place; no IoStore output |
| `--kawaii-physics-usmap <PATH>` | auto-download if omitted | mapping file |
| `--game-paks-dir <DIR>` | saved GUI config if available | deps for repacking IoStore |
| `--full-iostore-check` | off | slow all-container dependency path |

Output rules:

| Behavior | Rule |
| --- | --- |
| suffix | appends `_9999999_P` unless `--no-mod-suffix` |
| generated files | `.pak`, `.utoc`, `.ucas` |
| generated `.pak` | fake pak containing `chunknames` |
| IoStore input without `--kawaii-physics` | copied/installed, not converted |
| IoStore input with `--kawaii-physics` | to-legacy temp extraction, Kawaii port, re-pack |

## `fix-kawaii-physics`

```console
retoc-rivals-cli fix-kawaii-physics
retoc-rivals-cli fix-kawaii-physics --output fixed-mods
retoc-rivals-cli fix-kawaii-physics --usmap mappings.usmap
```

| Option | Default | Meaning |
| --- | --- | --- |
| `-o, --output <DIR>` | `fixed-mods` | rebuilt mod output directory |
| `-u, --usmap <PATH>` | saved config, then auto-download | mapping file |

Uses saved GUI config for installed mods dir and game `Paks` dir.

## Recipes

| Task | Command |
| --- | --- |
| inspect archive | `retoc-rivals-cli info SomeMod.zip` |
| generate filters | `retoc-rivals-cli manifest SomeMod_9999999_P.utoc --filters` |
| extract IoStore | `retoc-rivals-cli unpack SomeMod_9999999_P.utoc --output unpacked` |
| batch extract downloads | `retoc-rivals-cli unpack-dir "C:\Downloads\Rivals Mods"` |
| rebuild edited folder | `retoc-rivals-cli pack unpacked --output "C:\Path\To\~mods"` |
| in-place Kawaii fix | `retoc-rivals-cli pack unpacked --kawaii-physics-only` |
