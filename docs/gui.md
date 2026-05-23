# GUI

`repak-gui` is default user workflow for Marvel Rivals mods: install, convert, inspect, organize, enable/disable, delete, and launch.

## Capabilities

| Area | Details |
| --- | --- |
| Inputs | `.pak`, IoStore triples (`.pak/.utoc/.ucas`), `.zip`, `.rar`, raw asset folders |
| Conversion | legacy pak -> current output; raw folder -> IoStore; optional obfuscation |
| KawaiiPhysics | ports assets during conversion when `.usmap` configured |
| Management | enable/disable/delete installed mods; list `~mods`; live refresh |
| Organization | search by name/path/category/tag; add/remove tags |
| Inspection | pak metadata, file table, search, copy path/offset/hash, extract where supported |
| Launch | start Marvel Rivals through detected Steam install |

## Main Window

| UI Area | Contains |
| --- | --- |
| Top bar | `File`, `Settings`, `Donate`, current mod folder, `Browse`, `Open mod folder`, `Launch Game` |
| Left panel | installed mod list, search, category/tag filters, selected mod actions |
| Center panel | package file table, file search, path/offset/size/chunk columns, context actions |
| Details panel | mount point, path hash seed, pak version, category, character info, obfuscation, file count |

## Install Inputs

| Input | GUI behavior |
| --- | --- |
| `.pak` only | legacy pak; can install/copy or repack depending options |
| `.pak` + `.utoc` + `.ucas` | IoStore mod set; copies or repacks depending options |
| `.utoc` / `.ucas` | resolved by same-stem companion files next to it |
| `.zip` / `.rar` | extracted then scanned for supported mod payloads |
| raw folder | converted to IoStore unless mod type routes to pak-only path |

## Install Flow

1. Drag files/folders into window, or use `File -> Install mods` / `File -> Pack folder`.
2. Confirm each install row is enabled.
3. Check detected category/type.
4. Adjust output name and options.
5. Click `Install mod`.

For IoStore inputs installed with `To repak`, progress is split across legacy
extraction and IoStore rebuild. KawaiiPhysics work runs during the rebuild phase,
so the bar should no longer reach 100% immediately after extraction.

## Install Options

| Option | Meaning |
| --- | --- |
| `Enabled` | include row in install |
| `To repak` | convert legacy pak/IoStore into current output form |
| `Kawaii porter` | apply KawaiiPhysics conversion during install |
| `Obfuscate` | obfuscate generated IoStore containers |
| `Mount point` | pak mount point; usually `../../../` |
| `Path hash seed` | pak path hash seed; usually `00000000` |
| `Compression Algorithm` | compression for generated output |

## Installed Mod Management

| Action | Notes |
| --- | --- |
| Search/filter | name, path, category, character, tag |
| Enable/disable | renames package extensions, e.g. `.pak` <-> disabled extension |
| Delete | removes selected mod and companion `.utoc/.ucas` when present |
| Tags | local metadata for grouping/testing/troubleshooting |
| Extract/convert | available from selected-mod actions; IoStore needs container context |

## KawaiiPhysics Mapping

KawaiiPhysics porting needs a `.usmap`.

| Menu | Action |
| --- | --- |
| `File -> Select Mapping file` | set mapping |
| `File -> Clear Mapping file` | clear mapping |

If Kawaii porter is enabled without mapping, install blocks until mapping is selected.

## Recommended Workflows

Downloaded archive:

```text
Open GUI -> confirm ~mods -> drag archive -> review rows/options -> install -> launch game
```

Raw asset folder:

```text
File -> Pack folder -> select folders -> review names/options -> install
```

Conflict testing:

```text
Tag related mods -> filter by tag -> disable/enable groups -> launch game
```
