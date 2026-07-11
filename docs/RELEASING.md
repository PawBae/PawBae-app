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
keypair first (see "Updater signing"). macOS tag builds additionally refuse
to start while any of the six Apple signing secrets is missing (see
"macOS signing & notarization").

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

## macOS signing & notarization

Tag builds sign the app with a **Developer ID Application** certificate and
notarize it through Apple's notary service — both handled by the Tauri
bundler once the six secrets below exist. The release workflow hard-gates on
them: a `v*` tag build fails fast while any secret is missing. Manual
`workflow_dispatch` smoke builds skip signing while the secrets are absent
(unsigned artifacts open via right-click → Open or
`xattr -cr /Applications/PawBae.app`), and sign + notarize once they exist.

One-time provisioning, ~20 minutes, needs the active Apple Developer Program
membership:

### 1. Create the Developer ID Application certificate

1. On your Mac: **Keychain Access → Certificate Assistant → Request a
   Certificate From a Certificate Authority…** — your Apple ID email, "Saved
   to disk", save the `.certSigningRequest` file.
2. At <https://developer.apple.com/account/resources/certificates/add> pick
   **Developer ID Application**, upload the CSR, download the `.cer`, and
   double-click it so it joins its private key in the login keychain.

### 2. Export the .p12 and collect the identity strings

Keychain Access → My Certificates → right-click the new
"Developer ID Application: …" entry → Export…, format **.p12**, choose an
export password. Then:

```bash
base64 -i DeveloperID.p12 | pbcopy         # → APPLE_CERTIFICATE (now on the clipboard)
security find-identity -v -p codesigning   # → the full "Developer ID Application: Name (TEAMID)" string
```

The Team ID is the 10-character code in the identity string's parentheses
(also on <https://developer.apple.com/account> under Membership details).

### 3. App-specific password for notarization

<https://account.apple.com> → Sign-In and Security → App-Specific Passwords →
generate one (name it e.g. `pawbae-notarize`; format `xxxx-xxxx-xxxx-xxxx`).

### 4. Store the six secrets

```bash
pbpaste | gh secret set APPLE_CERTIFICATE --repo PawBae/PawBae-app
gh secret set APPLE_CERTIFICATE_PASSWORD --repo PawBae/PawBae-app  # .p12 export password
gh secret set APPLE_SIGNING_IDENTITY --repo PawBae/PawBae-app      # "Developer ID Application: Name (TEAMID)"
gh secret set APPLE_ID --repo PawBae/PawBae-app                    # Apple ID email
gh secret set APPLE_PASSWORD --repo PawBae/PawBae-app              # app-specific password from step 3
gh secret set APPLE_TEAM_ID --repo PawBae/PawBae-app               # 10-char Team ID
rm DeveloperID.p12  # GitHub secrets is its only durable home
```

### 5. Verify

Run the release workflow manually (`workflow_dispatch`): with the secrets in
place even smoke builds sign + notarize. Download a macOS artifact and check
(expected outputs in comments):

```bash
codesign -dv --verbose=2 PawBae.app   # Authority=Developer ID Application: …
spctl -a -t open --context context:primary-signature -vv PawBae_*.dmg
                                      # accepted · source=Notarized Developer ID
xcrun stapler validate PawBae.app     # The validate action worked!
```

Notes:

- Notarization adds roughly 2–10 minutes per macOS matrix leg.
- Losing this certificate is recoverable (revoke, reissue, re-set the
  secrets) — unlike the minisign key, nothing already shipped stops updating.
- **Windows**: still unsigned for v1 — SmartScreen shows "unknown publisher".
  Signing needs an OV/EV code-signing certificate; see
  <https://tauri.app/distribute/sign/windows/>
