# Release Automation

`.github/workflows/nexus.yml` publishes GitHub release assets to Nexus Mods via the Nexus Mods v3 API.

## Flow

1. Trigger on GitHub release.
2. Download release assets.
3. Convert `.tar.xz` assets to `.zip`.
4. Map output filenames to Nexus file update groups.
5. Upload each file as a new Nexus version.

## Required GitHub Config

| Type | Name | Meaning |
| --- | --- | --- |
| secret | `NEXUSMODS_API_KEY` | Nexus Mods API key |
| variable | `NEXUS_FILE_GROUP_MAP_JSON` | final upload filename -> Nexus file update group ID |

## Optional GitHub Config

| Type | Name | Meaning |
| --- | --- | --- |
| variable | `NEXUS_FILE_NAME_MAP_JSON` | final upload filename -> Nexus display name |
| variable | `NEXUS_FILE_CATEGORY` | Nexus category: `main`, `optional`, or `miscellaneous`; defaults to `main` |

## Example Group Map

```json
{
  "repak-gui-x86_64-pc-windows-msvc.zip": "12345",
  "repak-gui-x86_64-unknown-linux-gnu.zip": "67890"
}
```

## Notes

| Constraint | Detail |
| --- | --- |
| filename matching | keys must match final uploaded filenames after conversion |
| update groups | upload creates new version in existing Nexus file group |
| categories | invalid category values fail upload |
