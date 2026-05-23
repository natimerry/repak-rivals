# v3.2.4

## Changes:
- Fix KawaiiPhysics batch conversion so package-id collisions across mod variants do not repackage the first matching mod repeatedly.
- Process nested `.7z`, `.rar`, and `.zip` archives when `retoc-rivals-cli pack` is given a directory of archived mods.
- Add `retoc-rivals-cli pack-dir` for mixed folders containing raw mod directories, IoStore triples, legacy paks, and archives.
- Batch IoStore extraction for directory/archive pack inputs so game containers are opened once per input set instead of once per archive.
- Log mixed-directory scans and archive extraction progress so long `.7z` operations are visible in the CLI.
- Move in-place KawaiiPhysics asset patching from `pack --kawaii-physics-only` to `fix-kawaii-physics <dir>`.

# v3.2.3

## Changes:
- Replace per-asset KawaiiPhysics helper process spawning with a hosted managed DLL binding.
- Fix IoStore `To repak` progress accounting so extraction and rebuild/Kawaii work are both represented.
- Speed up KawaiiPhysics installs by caching the parsed USMAP in the managed binding and skipping non-Kawaii assets before entering the binding.
- Add `retoc-rivals-cli` for inspecting, manifesting, unpacking, packing, and fixing Marvel Rivals IoStore/legacy pak mods from the command line.
- Add CLI and GUI documentation covering build setup, GUI usage, release automation, troubleshooting, and `retoc-rivals-cli` workflows.
- Share Marvel Rivals path detection and latest-depot USMAP resolution between the GUI and CLI.
- Add `.7z` archive support for GUI drag-and-drop installs and `retoc-rivals-cli` archive inputs.
- Remove legacy `repak-cli`

# Version 3.1.0 (2026-05-18)

This release focuses on Repak GUI responsiveness, install progress reporting, and release-build Windows launch behavior.

## Changes:
- Fix heavy GUI lag when large IoStore mods are present by avoiding expensive per-frame detail work.
- Fix install progress so it is based on the total number of assets across all queued mods instead of completing after the first mod.
- Add retoc logging hooks so normal To Zen installs report through app logging instead of spawning a console.
- Keep explicit console progress for To Legacy conversions, where long-running work needs visible feedback.
- Reduce noisy retoc material tag logs during normal GUI installs.
- Fix release-build Windows folder opening by using native shell open behavior instead of fragile `explorer.exe` process handling.
- Fix release-build game launching by opening the Steam protocol through the Windows shell, avoiding OS error 50 failures.
- Fix To Legacy filter handling and clean up related Rust warnings.
- Improve mod list/file table performance for large mods.

# Version 2.11.4 (2026-04-05)

- Fix `character_data.json` parsing from markdown files

# Version 2.11.3 (2026-04-05)

- Add the ability to extract both `.pak` and `IOStore` mods from the GUI right click

# Version 2.11.2 (2026-04-05)

- Fetch updated character ids from table
- Improve logging system

# Version 2.11.1 (2026-04-04)

This release improves mod list usability and suffix handling.

## Changes:
- Updated `character_data.json`

# Version 2.11.0 (2026-04-04)

This release improves mod list usability and suffix handling.

## Changes:
- Add search to the GUI file table.
- Add search to the mod files pane.
- Strip `_9999999_P` / `_999999_P` from displayed mod names in the mod pane only.
- Add a small gray `[9999]` indicator for mods that already include the suffix.
- Ensure install/pack output names append `_9999999_P` when missing.
- Restore mod row label highlight styling after suffix-indicator UI updates.

# Version 2.10.2 (2026-04-04)

This release updates compression and texture handling.

## Changes:
- Switch Oodle compression from `Selkie` to `Kraken`.
- Fix blurry texture due to improper `FBulkDataMapEntry` handling in `retoc-rivals` submodule.
- Write `latest.log` next to the executable instead of the current working directory.
- Adjust Nexus release publishing so CLI releases are ignored.

# Version 2.9.0 (2026-03-23)

This release improves extraction and release automation.

## Changes:
- Add the ability to extract `.utoc` files from the GUI file table.
- Rewrite and simplify the README around the GUI-first workflow.
- Add a Nexus Mods release publishing workflow.

# Version 2.8.2 (2026-03-11)

This release improves offline behavior and adds Nix support.

## Changes:
- Make internet access optional for release update checks.
- Add and refine `flake.nix` and `flake.lock` for Nix-based builds.
- Add Nexus workflow scaffolding for release publishing.

# Version 2.8.1 (2026-02-06)

## Changes:
- Fix the repak GUI update check logic.

# Version 2.8.0 (2026-02-06)

This release tightens update handling for release builds.

## Changes:
- Make update prompts mandatory in non-debug builds.
- Rework version parsing for release checks.

# Version 2.5.4 (2025-05-06)

## Changes:
- Fix removal of tempdir causes issues in install mods


# Version 2.5.2 (2025-05-06)

This release contains a window asking users to donate to the project.

## Changes:
- Clean up temporary directories after creating them.

# Version 2.5.1 (2025-05-06)

## New features:
- Ability to install mods from zip files directly
- Show packaging type in install window
- Allow users to unselect specific mods when installing in batch

# Version 2.4.0 (2025-05-05)

This release contains code simplification and bug fixes.

## Changes:
- Added ability to fix dragged .zip/.rar files containing one or more `.pak` files into repak gui 

# Version 2.4.0 (2025-05-04)

This release contains QOL improvements and movie mod fixes for the mod manager.

## Changes:
- Simplify mod type detection
- Add mod type detection for IOStore mods
- Add emma frost skin names to mod categories

## What's broken
- Modtype while importing zip / rar files still doesnt work. This requires extra work

# Version 2.3.0 (2025-05-04)

This release contains QOL improvements and movie mod fixes for the mod manager.

Changes:
 - Removed option to set audio mod manually, this is done automatically for audio mods now.
 - Make movie mods pak the same way as audio mods
 - Remove retrieving filenames from chunkname, instead use the iostore manifest
