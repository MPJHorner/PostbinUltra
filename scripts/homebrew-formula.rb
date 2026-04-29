# Homebrew Cask formula scaffold for `brew install --cask postbin-ultra`.
#
# This file is a TEMPLATE. The actual tap lives at MPJHorner/homebrew-postbin
# (a separate repo). On every release, the SHA256 + version below need to be
# bumped to point at the new .dmg artefacts. See `scripts/bump-tap.sh` once
# the tap repo is created.
#
# Until then, users install from the .dmg manually (see install.sh).

cask "postbin-ultra" do
  arch arm: "aarch64-apple-darwin", intel: "x86_64-apple-darwin"

  version "2.0.0"

  on_arm do
    sha256 "REPLACE_ME_ARM64_DMG_SHA256"
  end

  on_intel do
    sha256 "REPLACE_ME_X86_64_DMG_SHA256"
  end

  url "https://github.com/MPJHorner/PostbinUltra/releases/download/v#{version}/PostbinUltra-#{version}-#{arch}.dmg",
      verified: "github.com/MPJHorner/"
  name "Postbin Ultra"
  desc "Native HTTP request inspector with forward + replay history"
  homepage "https://mpjhorner.github.io/PostbinUltra/"

  livecheck do
    url :url
    strategy :github_latest
  end

  app "PostbinUltra.app"

  zap trash: [
    "~/Library/Application Support/PostbinUltra",
    "~/Library/Preferences/co.uk.matthorner.postbin-ultra.plist",
    "~/Library/Saved Application State/co.uk.matthorner.postbin-ultra.savedState",
  ]
end
