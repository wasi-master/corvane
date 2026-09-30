# Homebrew cask for Corvane — lives in the `wasi-master/homebrew-corvane` tap
# as `Casks/corvane.rb`. `packaging/release.sh` rewrites `version` and `sha256`.
#
#   brew install --cask wasi-master/corvane/corvane --no-quarantine
#
# `--no-quarantine` matters: the bundle is self-signed (no Apple Developer
# ID), so a quarantined copy is blocked by Gatekeeper on first launch.
cask "corvane" do
  version "0.1.0"
  sha256 "0000000000000000000000000000000000000000000000000000000000000000"

  url "https://github.com/wasi-master/corvane/releases/download/v#{version}/Corvane-#{version}-macos-universal.zip"
  name "Corvane"
  desc "Native GitHub Desktop clone in Rust (GPUI + gitoxide)"
  homepage "https://github.com/wasi-master/corvane"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: ">= :sequoia"

  app "Corvane.app"
  # `corvane [open] [path]` / `corvane clone <url>` (Install Command Line Tool… does the same by hand)
  binary "#{appdir}/Corvane.app/Contents/Resources/corvane"

  zap trash: [
    "~/Library/Application Support/Corvane",
    "~/Library/Caches/Corvane",
    "~/Library/Logs/Corvane",
    "~/Library/Preferences/com.wasimaster.corvane.plist",
    "~/Library/Saved Application State/com.wasimaster.corvane.savedState",
  ]

  caveats <<~EOS
    Corvane is self-signed. Install with --no-quarantine, or after a plain
    install allow it once under System Settings › Privacy & Security › Open Anyway.
    Updates for this install come from `brew upgrade corvane`; the in-app
    updater only points there.
  EOS
end
