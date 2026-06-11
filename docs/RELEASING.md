# Releasing PawBae

## 1. Bump the version

The version lives in three files — keep them in sync:

| File | Field |
| --- | --- |
| `src-tauri/tauri.conf.json` | `version` (canonical — bundler + updater read this) |
| `package.json` | `version` |
| `src-tauri/Cargo.toml` | `version` |

Update `CHANGELOG.md` with the changes, then land the bump via a PR (never
push to `main` directly).

## 2. Tag and build

```bash
git checkout main && git pull
git tag vX.Y.Z
git push origin vX.Y.Z
```

The tag push triggers `.github/workflows/release.yml`, which builds:

- `PawBae_X.Y.Z_aarch64.dmg` — macOS Apple Silicon
- `PawBae_X.Y.Z_x64.dmg` — macOS Intel
- `PawBae_X.Y.Z_x64-setup.exe` — Windows NSIS installer

and uploads them to a **draft** GitHub Release named `PawBae vX.Y.Z`.

To verify the pipeline without cutting a release, run the workflow manually
(`workflow_dispatch`) — bundles are attached as workflow artifacts instead.

## 3. Publish

1. Open the draft release on GitHub, sanity-check the three assets
   (download one, install, launch).
2. Write the release notes and publish.

## 4. Update the in-app updater manifest

The app checks `https://pawbae.ai/update/latest.json` (not GitHub) — see
`src-tauri/src/commands/update.rs`. After publishing, update that file in the
website repo with the new version and the GitHub asset URLs:

```json
{
  "platforms": {
    "macos": {
      "version": "X.Y.Z",
      "url": "https://github.com/PawBae/PawBae-app/releases/download/vX.Y.Z/PawBae_X.Y.Z_aarch64.dmg",
      "notes_i18n": { "en": "…", "zh": "…" }
    },
    "windows": {
      "version": "X.Y.Z",
      "url": "https://github.com/PawBae/PawBae-app/releases/download/vX.Y.Z/PawBae_X.Y.Z_x64-setup.exe",
      "notes_i18n": { "en": "…", "zh": "…" }
    }
  }
}
```

Users only see the update prompt once this manifest changes, so a bad build
can sit published on GitHub without being pushed to anyone.

> Note: the macOS manifest currently points Intel users at the aarch64 DMG —
> the manifest has one `macos` slot. If Intel uptake matters, extend
> `check_for_update` to pick per-arch URLs before advertising updates to
> Intel users.

## Code signing (not set up yet)

Builds are currently **unsigned**:

- **macOS**: Gatekeeper warns on first open. Users must right-click → Open, or
  run `xattr -cr /Applications/PawBae.app`. To sign + notarize, add the Apple
  Developer secrets (`APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`,
  `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`) and
  uncomment the env block in `release.yml`. See
  https://tauri.app/distribute/sign/macos/
- **Windows**: SmartScreen shows "unknown publisher". Signing needs a code
  signing certificate; see https://tauri.app/distribute/sign/windows/
