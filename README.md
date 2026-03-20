# repak-rivals

`repak-rivals` is a Marvel Rivals mod packaging and install toolset.

Use `repak-gui` by default. It is the intended workflow for current Marvel Rivals mods.

`repak_cli` is mainly for pak inspection and pak-only workflows. It does not generate `.utoc` / `.ucas`, is not the preferred option for modern Rivals mods, and should only be used if you already understand the packaging constraints.

## Install

1. Download the latest release from Nexus Mods: https://www.nexusmods.com/marvelrivals/mods/1717
2. Launch `repak-gui`.
3. Confirm the detected Marvel Rivals mod folder.
4. Drag in mods, archives, or mod folders, or use `File -> Install mods` / `File -> Pack folder`.
5. Install the generated output into the `~mods` directory.

## Features

- GUI-first workflow for Marvel Rivals mod installation
- drag-and-drop support for `.pak`, `.zip`, `.rar`, and mod folders
- automatic detection of the Marvel Rivals mod directory
- IOStore generation for modern mods
- pak repacking for audio and movie patches
- mesh-fix support for custom model mods
- batch install flow with per-mod options
- installed mod browser with enable, disable, and delete actions
- pak content viewer with extract, copy path, copy offset, and hash actions
- mod type detection for character, UI, audio, and movie mods
- Windows and Linux release builds

## Latest Changelog

### v2.8.2 - 2026-03-11

- made internet-dependent behavior optional
- improved `flake.nix` support

### v2.8.1 - 2026-02-06

- fixed the GUI update check

### v2.8.0 - 2026-02-06

- made update checks mandatory
- refreshed skin data
- moved mesh directory fetching into the background

### v2.7.0 - 2026-02-05

- added release-mode fetching for latest skin data

<details>
<summary>CLI Help</summary>

```console
$ repak --help
Usage: repak [OPTIONS] <COMMAND>

Commands:
  info       Print .pak info
  list       List .pak files
  hash-list  List .pak files and the SHA256 of their contents. Useful for finding differences between paks
  unpack     Unpack .pak file
  pack       Pack directory into .pak file
  get        Reads a single file to stdout
  help       Print this message or the help of the given subcommand(s)

Options:
  -a, --aes-key <AES_KEY>  256 bit AES encryption key as base64 or hex string if the pak is encrypted [default: 0C263D8C22DCB085894899C3A3796383E9BF9DE0CBFB08C9BF2DEF2E84F29D74]
  -h, --help               Print help
  -V, --version            Print version
```

</details>
