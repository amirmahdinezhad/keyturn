cask "keyturn" do
  version "1.1.0"
  sha256 "REPLACE_WITH_DMG_SHA256"

  url "https://github.com/amirmahdinezhad/keyturn/releases/download/v#{version}/Keyturn_#{version}_universal.dmg",
      verified: "github.com/amirmahdinezhad/keyturn/"
  name "Keyturn"
  desc "Menubar API-key manager and quota monitor for AI coding CLIs"
  homepage "https://github.com/amirmahdinezhad/keyturn"

  livecheck do
    url :url
    strategy :github_latest
  end

  app "Keyturn.app"

  zap trash: [
    "~/Library/Application Support/ai.opencode.desktop/agent-rtl.keys",
    "~/Library/Application Support/ai.opencode.desktop/agent-rtl.keys.*",
    "~/Library/Application Support/ai.opencode.desktop/agent-rtl.profiles.json",
  ]
end
