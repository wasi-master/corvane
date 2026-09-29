# Homebrew tap layout

The primary install path (PLAN.md §2, §3.8) is a Homebrew cask in a tap
repository named `homebrew-corvane` under the `wasi-master` account. This
folder holds the tap's contents so a release can copy them over:

```
homebrew-corvane/            # github.com/wasi-master/homebrew-corvane
├─ README.md                 # the install instructions below
└─ Casks/
   └─ corvane.rb             # packaging/homebrew/Casks/corvane.rb
```

Creating the tap repository is a manual step (nothing here creates or pushes
remotes). Once it exists:

```bash
cp packaging/homebrew/Casks/corvane.rb ../homebrew-corvane/Casks/corvane.rb
cp packaging/homebrew/README.md ../homebrew-corvane/README.md
(cd ../homebrew-corvane && git commit -am "corvane <version>" && git push)
```

## Installing Corvane

```bash
brew install --cask wasi-master/corvane/corvane --no-quarantine
```

`--no-quarantine` is required: Corvane is ad-hoc signed (a hobby project
without an Apple Developer ID), and macOS 15 blocks a quarantined,
unnotarized app on first launch. Without the flag, open it once through
System Settings › Privacy & Security › Open Anyway, or run
`xattr -d com.apple.quarantine /Applications/Corvane.app`.

Upgrade with `brew upgrade corvane`; the in-app updater recognises a
Homebrew install and only points at that command.

## Updating the cask for a release

`packaging/release.sh` prints the zip's sha256 and, with `UPDATE_CASK=1`,
rewrites `version` and `sha256` in `packaging/homebrew/Casks/corvane.rb`.
Then copy the file to the tap and push. `brew audit --cask corvane` and
`brew install --cask ./Casks/corvane.rb --no-quarantine` check it locally.
