# Releasing Corvane

How a tagged commit becomes the artefacts the self-updater and the Homebrew
cask consume (PLAN.md §3.8). Nothing here needs an Apple Developer ID: the
bundle is signed with a self-signed certificate and integrity comes from
minisign.

## Signing keys (one-time, maintainer's machine)

The self-updater verifies every download against an Ed25519 minisign public
key compiled into the binary. Generate the pair once:

```bash
brew install minisign
minisign -G -p corvane-release.pub -s ~/.minisign/corvane-release.key
```

(`rsign2` from crates.io is a drop-in alternative: `rsign generate -p … -s …`.)

- `corvane-release.pub` is public. Copy it to `packaging/corvane-release.pub`
  and commit it; `packaging/release.sh` exports its key line as
  `CORVANE_UPDATE_PUBLIC_KEY` before building.
- `~/.minisign/corvane-release.key` is the **secret key**. It never enters
  the repository or the bundle. Keep it in the password manager, and store
  its contents in the GitHub Actions secret `MINISIGN_SECRET_KEY` (with the
  passphrase in `MINISIGN_PASSWORD`) for CI signing.
- Losing the secret key means shipping a new public key in a release users
  install by hand: existing installs cannot verify anything signed with a
  new key.

Every build without `CORVANE_UPDATE_PUBLIC_KEY` prints
`warning: corvane-platform@…: CORVANE_UPDATE_PUBLIC_KEY is not set…` from
`crates/corvane-platform/build.rs` and compiles a placeholder key that
rejects every real update (`corvane_platform::updater::PUBLIC_KEY`).
Development builds never check for updates anyway
(`corvane_core::updater::updates_enabled`, `CORVANE_UPDATE_CHECK=1` forces
it).

## Code-signing certificate (one-time, maintainer's machine)

macOS lets an app read its Keychain items (the account tokens,
`corvane_platform::keychain`) without asking only while its designated
requirement matches the one that created them. An ad-hoc signature's
requirement is the binary's hash, so every build and every update would ask
for the login password again. Corvane is therefore signed with a
self-signed certificate: its requirement, `identifier
"com.wasimaster.corvane" and certificate leaf = H"…"`, stays the same across
builds. Gatekeeper treats it like an ad-hoc signature (the `--no-quarantine`
advice stands).

```bash
packaging/signing-cert.sh create   # → login keychain, ~/.corvane-signing/corvane-signing.{p12,password}
```

- Keep the `.p12` and its password in the password manager and add them as
  the GitHub Actions secrets `MACOS_SIGNING_P12` (base64 of the file) and
  `MACOS_SIGNING_P12_PASSWORD`. On another machine:
  `packaging/signing-cert.sh import corvane-signing.p12`.
- `packaging/bundle.sh` signs with it whenever the keychain has it (the
  first run asks to use the key: Always Allow), else ad-hoc;
  `CORVANE_SIGN_IDENTITY=-` forces ad-hoc. `packaging/release.sh` refuses a
  bundle that is not signed with it (`ALLOW_ADHOC=1` for tests).
- Losing it is not fatal, unlike the minisign key: a new certificate means
  one more password prompt per Keychain item on every install. The first
  self-signed release does the same for installs of ad-hoc builds.
- `codesign -d -r- /Applications/Corvane.app` shows the requirement.

## Release checklist

1. Bump `version` in `Cargo.toml` (`[workspace.package]`), run
   `packaging/acknowledgements.sh` if dependencies changed, commit,
   `git tag v<version>`.
2. `packaging/release.sh` (see `packaging/release.sh --help`): release build
   with the public key, `packaging/bundle.sh release`, the universal binary
   when both target directories exist, `Corvane-<version>-macos-universal.zip`
   (+ `.dmg`), the `Corvane-Full-…` variant when `FULL=1`, minisign
   signatures for every asset, the packs manifest, and the cask's sha256.
   Output lands in `target/release-assets/`.
3. Create the GitHub release for the tag and upload everything in
   `target/release-assets/`. The self-updater reads
   `GET /repos/wasi-master/corvane/releases/latest`, picks the `.zip` whose
   name contains `universal` (else the machine's architecture, else `macos`)
   and needs `<zip>.minisig` next to it. The release body is Markdown; list
   items tagged `[New]` / `[Improved]` / `[Fixed]` / `[Added]` / `[Removed]`
   become the Release Notes dialog's entries (`corvane_core::release_notes`).
4. Update `packaging/homebrew/Casks/corvane.rb` with the version and the
   sha256 `release.sh` printed, and push it to the `homebrew-corvane` tap
   (`packaging/homebrew/README.md`).
5. Upload `packs-manifest.json` (+ `.minisig`) and the pack archives to the
   same release when a pack changed (`crates/corvane-packs`).

## Signing in CI

`.github/workflows/release.yml` (to be added with the first tagged release)
runs on `macos-15`. Before `packaging/release.sh`, import the code-signing
certificate into a temporary keychain (`.github/workflows/ci.yml` does the
same for its bundle artifact):

```yaml
- name: Code-signing certificate
  env:
    MACOS_SIGNING_P12: ${{ secrets.MACOS_SIGNING_P12 }}
    MACOS_SIGNING_P12_PASSWORD: ${{ secrets.MACOS_SIGNING_P12_PASSWORD }}
  run: packaging/signing-cert.sh ci
```

and after it sign the assets:

```yaml
- name: Sign release assets
  env:
    MINISIGN_SECRET_KEY: ${{ secrets.MINISIGN_SECRET_KEY }}
    MINISIGN_PASSWORD: ${{ secrets.MINISIGN_PASSWORD }}
  run: |
    brew install minisign
    umask 077
    printf '%s' "$MINISIGN_SECRET_KEY" > "$RUNNER_TEMP/corvane-release.key"
    for asset in target/release-assets/*.zip target/release-assets/*.dmg target/release-assets/packs-manifest.json; do
      printf '%s\n' "$MINISIGN_PASSWORD" | minisign -S -s "$RUNNER_TEMP/corvane-release.key" \
        -t "corvane $GITHUB_REF_NAME $(basename "$asset")" -m "$asset"
    done
    rm -f "$RUNNER_TEMP/corvane-release.key"
```

The build step exports the public key the same way `release.sh` does:

```yaml
- run: echo "CORVANE_UPDATE_PUBLIC_KEY=$(tail -n1 packaging/corvane-release.pub)" >> "$GITHUB_ENV"
```

## How the updater uses the assets

`corvane_platform::updater` (checked at launch with a 15–60 s jitter and
every four hours, release builds only):

1. `GET …/releases/latest` (no token; `CORVANE_UPDATE_FEED` overrides the
   URL for testing). A tag newer than the running version by semver wins;
   drafts are skipped.
2. `<zip>` and `<zip>.minisig` are downloaded to
   `~/Library/Caches/Corvane/updates/`; the zip is verified with the
   compiled-in key (BLAKE2b-prehashed minisign, no legacy mode).
3. The "Corvane N is available" banner appears. "Install and Restart" (banner,
   Release Notes dialog, About) unpacks the zip with `ditto` (keeps the
   signature), renames the running bundle to `Corvane.app.old`, moves
   the new bundle in, schedules `open -n` for after this process exits and
   quits. The next launch deletes `Corvane.app.old`.
4. A bundle under `/opt/homebrew/Caskroom`, `/usr/local/Caskroom`, or in
   `/Applications` while a `corvane` cask is installed, is never swapped: the
   banner says `brew upgrade corvane`.

Files the app downloads itself carry no quarantine attribute, so the self-signed
update launches without Gatekeeper's "Open Anyway" dance (PLAN.md R5).

## Testing the flow locally

```bash
# a throwaway key pair (never reuse for a real release)
rsign generate -W -p /tmp/test.pub -s /tmp/test.key
CORVANE_UPDATE_PUBLIC_KEY="$(tail -n1 /tmp/test.pub)" cargo build -p corvane && packaging/bundle.sh
# a "new" version: copy the bundle, bump CFBundleShortVersionString, re-sign, zip, sign
cp -R target/bundle/Corvane.app /tmp/new/ && /usr/libexec/PlistBuddy -c 'Set :CFBundleShortVersionString 9.9.9' /tmp/new/Corvane.app/Contents/Info.plist
codesign --force --sign "${CORVANE_SIGN_IDENTITY:--}" /tmp/new/Corvane.app
(cd /tmp/new && ditto -c -k --sequesterRsrc --keepParent Corvane.app Corvane-9.9.9-macos-universal.zip)
rsign sign -W -s /tmp/test.key -x /tmp/new/Corvane-9.9.9-macos-universal.zip.minisig /tmp/new/Corvane-9.9.9-macos-universal.zip
# a fake feed: latest.json with tag_name "v9.9.9" and assets pointing at http://127.0.0.1:8765/
(cd /tmp/new && python3 -m http.server 8765)
CORVANE_UPDATE_CHECK=1 CORVANE_UPDATE_FEED=http://127.0.0.1:8765/latest.json open -n target/bundle/Corvane.app
```

`CORVANE_POPUP=update-available[:brew]` shows the banner, About and Release
Notes with a sample update without any feed.
