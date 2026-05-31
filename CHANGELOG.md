# Changelog

## Version 3.4.0

### Changes

- Add Marvel Rivals `LODInfo.DefaultHiddenMaterials` patching through UAssetAPI for SkeletalMesh assets for use with MaterialTagPlugin.
- Add carrier autodetection for `LODHiddenMaterials` AssetUserData and inject the proprietary `DefaultHiddenMaterials: TArray<bool>` field into each `FSkeletalMeshLODInfo`.
- Add explicit bitmap override support for `DefaultHiddenMaterials`; each integer mask maps bit `n` to material slot `n`, with `1` meaning hidden by default.
- Split KawaiiPhysics porting from hidden-material patching in the GUI and CLI so `Fix KawaiiPhysics` / `--kawaii-physics` no longer silently patches hidden materials.
- Add GUI install option `Patch hidden mats` for normal install autodetection, separate from `Kawaii porter`.
- Add installed-mod right-click action `Patch Hidden Materials` with `Auto`, `Default`, and `Custom` mask modes.
- Add CLI flags `--patch-default-hidden-mats` for carrier autodetection and `--default-hidden-material-bitmaps <MASKS>` for explicit per-LOD masks.
- Document the default mask preset `0x0FFF0000,0x0FFF0000,0x0EFB0000` for right-click/default override workflows.

### Mask Usage

- GUI install: keep `Patch hidden mats` enabled to use carrier autodetection during normal install. There is no bitmap field in normal install.
- GUI installed mod right-click: use `Patch Hidden Materials`, then choose `Auto`, `Default`, or `Custom`.
- CLI carrier autodetect: pass `--patch-default-hidden-mats`.
- CLI explicit masks: pass `--default-hidden-material-bitmaps 0x0FFF0000,0x0FFF0000,0x0EFB0000`.
- Mask order is per LOD: first value is LOD0, second is LOD1, third is LOD2.
- Bit `n` controls material slot `n`; `1` means hidden by default, `0` means visible by default.
- Masks are `u64`, so they can directly set material slots `0` through `63`.
- A single mask is reused for every LOD. If fewer masks than LODs are provided, the final mask repeats.
- CLI masks accept comma-separated decimal or `0x` hex `u64` values. GUI `Custom` accepts comma, semicolon, or pipe separators.

## Version 3.3.0

### Changes

- Add an in-app `repak-gui` updater with release changelog display, staged install/restart, and a Settings menu action for manual update checks.
- Add normal and self-contained release variants for `repak-gui` and `retoc-rivals-cli`, with self-contained artifacts bundling the KawaiiPhysics .NET helper for users without a local .NET runtime.
- Add a cargo `xtask` release packager that builds self-contained artifacts per app and native CI target, while preventing Linux/Windows cross-target standalone builds.
- Append runtime-variant guidance to GitHub release notes and publish self-contained artifacts alongside cargo-dist archives.
- Improve KawaiiPhysics runtime failure handling in the GUI with a popup for installing .NET on Windows, viewing Linux package commands, or switching to self-contained `repak-gui`.
- Improve updater and KawaiiPhysics dialogs with fixed dark window styling, clearer progress/error states, and non-blocking background checks.
- Allow the KawaiiPhysics binding to roll forward to newer .NET runtimes and document the normal versus self-contained build behavior.
- Improve CLI and GUI error output for KawaiiPhysics and Zen conversion failures by preserving full error context.

## Version 3.2.4

### Changes

- Fix KawaiiPhysics batch conversion so package-id collisions across mod variants do not repackage the first matching mod repeatedly.
- Process nested `.7z`, `.rar`, and `.zip` archives when `retoc-rivals-cli pack` is given a directory of archived mods.
- Add `retoc-rivals-cli pack-dir` for mixed folders containing raw mod directories, IoStore triples, legacy paks, and archives.
- Batch IoStore extraction for directory/archive pack inputs so game containers are opened once per input set instead of once per archive.
- Log mixed-directory scans and archive extraction progress so long `.7z` operations are visible in the CLI.
- Move in-place KawaiiPhysics asset patching from `pack --kawaii-physics-only` to `fix-kawaii-physics <dir>`.

## Version 3.2.3

### Changes

- Replace per-asset KawaiiPhysics helper process spawning with a hosted managed DLL binding.
- Fix IoStore `To repak` progress accounting so extraction and rebuild/Kawaii work are both represented.
- Speed up KawaiiPhysics installs by caching the parsed USMAP in the managed binding and skipping non-Kawaii assets before entering the binding.
- Add `retoc-rivals-cli` for inspecting, manifesting, unpacking, packing, and fixing Marvel Rivals IoStore/legacy pak mods from the command line.
- Add CLI and GUI documentation covering build setup, GUI usage, release automation, troubleshooting, and `retoc-rivals-cli` workflows.
- Share Marvel Rivals path detection and latest-depot USMAP resolution between the GUI and CLI.
- Add `.7z` archive support for GUI drag-and-drop installs and `retoc-rivals-cli` archive inputs.
- Remove legacy `repak-cli`

## Version 3.1.0 (2026-05-18)

### Changes

- Fix heavy GUI lag when large IoStore mods are present by avoiding expensive per-frame detail work.
- Fix install progress so it is based on the total number of assets across all queued mods instead of completing after the first mod.
- Add retoc logging hooks so normal To Zen installs report through app logging instead of spawning a console.
- Keep explicit console progress for To Legacy conversions, where long-running work needs visible feedback.
- Reduce noisy retoc material tag logs during normal GUI installs.
- Fix release-build Windows folder opening by using native shell open behavior instead of fragile `explorer.exe` process handling.
- Fix release-build game launching by opening the Steam protocol through the Windows shell, avoiding OS error 50 failures.
- Fix To Legacy filter handling and clean up related Rust warnings.
- Improve mod list/file table performance for large mods.
