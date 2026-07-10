# Releasing PawBae

## 1. Bump the version

The version lives in three files — keep them in sync:

| File | Field |
| --- | --- |
| `apps/desktop/src-tauri/tauri.conf.json` | `version` (canonical — bundler + updater read this) |
| `apps/desktop/package.json` | `version` |
| `apps/desktop/src-tauri/Cargo.toml` | `version` |

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

and uploads them to a **draft** GitHub Release named `PawBae vX.Y.Z`. A
follow-up `sign` job then minisigns every asset, uploads the `.minisig` files
to the release, and prints ready-to-paste `"signature"` values in its Step
Summary (see §4 and "Updater signing" below).

Tag builds refuse to start while the updater public key is still the
placeholder or the `MINISIGN_SECRET_KEY` secret is missing — provision the
keypair first (see "Updater signing").

To verify the pipeline without cutting a release, run the workflow manually
(`workflow_dispatch`) — bundles are attached as workflow artifacts instead.

## 3. Publish

1. Open the draft release on GitHub, sanity-check the three assets
   (download one, install, launch).
2. Write the release notes and publish.

## 4. Update the in-app updater manifest

The app checks `https://pawbae.ai/update/latest.json` (not GitHub) — see
`apps/desktop/src-tauri/src/commands/update.rs`. After publishing, update that
file (website repo today; `apps/website/static/update/latest.json` after the
Vercel cutover) with the new version, the GitHub asset URLs, and the
**per-asset signatures**:

```json
{
  "platforms": {
    "macos": {
      "version": "X.Y.Z",
      "url": "https://github.com/PawBae/PawBae-app/releases/download/vX.Y.Z/PawBae_X.Y.Z_aarch64.dmg",
      "signature": "untrusted comment: …\nRW…\ntrusted comment: …\n…\n",
      "notes_i18n": { "en": "…", "zh": "…" }
    },
    "macos-x64": {
      "version": "X.Y.Z",
      "url": "https://github.com/PawBae/PawBae-app/releases/download/vX.Y.Z/PawBae_X.Y.Z_x64.dmg",
      "signature": "…",
      "notes_i18n": { "en": "…", "zh": "…" }
    },
    "windows": {
      "version": "X.Y.Z",
      "url": "https://github.com/PawBae/PawBae-app/releases/download/vX.Y.Z/PawBae_X.Y.Z_x64-setup.exe",
      "signature": "…",
      "notes_i18n": { "en": "…", "zh": "…" }
    }
  }
}
```

Slots: `macos` = Apple Silicon (historical name), `macos-x64` = Intel,
`windows` = NSIS installer. Intel Macs read only `macos-x64` — if the slot is
missing they simply see "up to date" (they must never receive the arm64 DMG).

`signature` is the full `.minisig` file content for that exact asset. Do NOT
build it by hand: the release workflow's `sign` job prints each value
JSON-escaped in its Step Summary — copy-paste from there. Verification-aware
app versions refuse to install an asset whose signature is missing or wrong,
so a manifest without signatures effectively disables updates.

Users only see the update prompt once this manifest changes, so a bad build
can sit published on GitHub without being pushed to anyone.

## Updater signing (minisign)

The updater verifies every downloaded installer against a minisign public key
compiled into the binary (`apps/desktop/src-tauri/updater-pubkey.pub`), so a
compromised website/CDN or GitHub release cannot push code to users. One-time
setup (owner only):

```bash
brew install minisign

# 1. Generate an UNENCRYPTED keypair (-W): the secret key's only home is
#    GitHub secrets, a passphrase stored next to it would add nothing.
minisign -G -W -p updater-pubkey.pub -s updater-secret.key

# 2. Store the secret key in GitHub secrets, then destroy the local copy.
gh secret set MINISIGN_SECRET_KEY --repo PawBae/PawBae-app < updater-secret.key
rm -P updater-secret.key

# 3. Commit the PUBLIC key into the repo (replaces the placeholder).
mv updater-pubkey.pub apps/desktop/src-tauri/updater-pubkey.pub
# → land via PR; the placeholder marker disappearing arms the release gate
```

Key rotation = repeat the steps; old app versions keep verifying with the old
key, so rotate only alongside a release and keep the old manifest entries
signed with the old key until adoption moves past that version.

Rules (non-negotiable):

- The secret key exists **only** in GitHub Actions secrets. Never in the repo,
  never in chat, never on a laptop after step 2.
- Losing the secret key = shipped binaries can never verify another update →
  users must manually download. Rotating after a leak has the same blast
  radius, so treat the secret with Apple-certificate-level care.

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
