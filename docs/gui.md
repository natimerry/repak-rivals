# GUI

`repak-gui` is default user workflow for Marvel Rivals mods: install, convert, inspect, organize, enable/disable, delete, and launch.

## Capabilities

| Area | Details |
| --- | --- |
| Inputs | `.pak`, IoStore triples (`.pak/.utoc/.ucas`), `.7z`, `.zip`, `.rar`, raw asset folders |
| Conversion | legacy pak -> current output; raw folder -> IoStore; optional obfuscation |
| KawaiiPhysics | ports assets during conversion when `.usmap` configured |
| Hidden materials | patches `LODInfo.DefaultHiddenMaterials` from carrier data during install, or from masks via installed-mod action |
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
| `.7z` / `.zip` / `.rar` | extracted then scanned for supported mod payloads |
| raw folder | converted to IoStore unless mod type routes to pak-only path |

## Install Flow

1. Drag files/folders into window, or use `File -> Install mods` / `File -> Pack folder`.
2. Confirm each install row is enabled.
3. Check detected category/type.
4. Adjust output name and options.
5. Click `Install mod`.

For IoStore inputs installed with `To repak`, progress is split across legacy
extraction and IoStore rebuild. KawaiiPhysics and hidden-material patching run
during the rebuild phase, so the bar should no longer reach 100% immediately
after extraction.

## Install Options

| Option | Meaning |
| --- | --- |
| `Enabled` | include row in install |
| `To repak` | convert legacy pak/IoStore into current output form |
| `Kawaii porter` | apply KawaiiPhysics conversion during install |
| `Patch hidden mats` | patch `LODInfo.DefaultHiddenMaterials` from carrier data during install |
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
| Fix KawaiiPhysics | right-click an installed IoStore mod; ports KawaiiPhysics only |
| Patch Hidden Materials | right-click an installed IoStore mod; patches `DefaultHiddenMaterials` only |

## Hidden-material modes in the right-click menu:

| Mode | Meaning |
| --- | --- |
| `Auto` | read `LODHiddenMaterials` carrier data and inject `DefaultHiddenMaterials`; supports `MaterialTagPlugin` and `RivalsMeshMaterialManager` / `HiddenMaterialsAssetUserData` carriers |
| `Default` | use the built-in mask preset `0x0FFF0000,0x0FFF0000,0x0EFB0000` |
| `Custom` | show a mask text box and `Creator` button; accepts decimal or `0x` hex masks separated by comma, semicolon, or pipe |

### Mask format

`DefaultHiddenMaterials` is a boolean array. Each material slot gets one boolean:

| Value | Meaning |
| --- | --- |
| `true` | material slot is hidden by default |
| `false` | material slot is visible by default |

Each mask is a `u64` bitfield for one LOD. Bit `0` controls material slot `0`,
bit `1` controls material slot `1`, and bit `2` controls material slot `2`.
A bit value of `1` writes `true`; a bit value of `0` writes `false`.
Slots `0` through `63` can be controlled directly.

Masks may be written in decimal or hexadecimal form. Separate multiple masks with commas.
```
0x0FFF0000,0x0FFF0000,0x0EFB0000
```
### LOD behavior
| Mask position | Applied to |
| ------------- | ---------- |
| First mask    | LOD 0      |
| Second mask   | LOD 1      |
| Third mask    | LOD 2      |
| ...           | ...        |

If fewer masks are provided than the asset has LODs, the final mask is reused for all remaining LODs.

The bitmap creator builds this string for you. In `Custom`, click `Creator`,
set the slot count, check the slots that should be `true`/hidden, and click
`Apply`.


## KawaiiPhysics Mapping

KawaiiPhysics porting and hidden-material patching need a `.usmap`.

| Menu | Action |
| --- | --- |
| `File -> Select Mapping file` | set mapping |
| `File -> Clear Mapping file` | clear mapping |

If `Kawaii porter` or `Patch hidden mats` is enabled without mapping, install blocks until mapping is selected.

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
