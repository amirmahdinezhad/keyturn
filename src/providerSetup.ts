/** Setup instructions checked against the providers' official guides. */
export interface ProviderSetup {
  installCommand: string;
  installSteps: string[];
  loginSteps: string[];
  verifyCommand?: string;
  installDocs: string;
  loginDocs: string;
}

/** Commands and instructions displayed by the setup wizard, never executed here. */
export const PROVIDER_SETUP: Record<string, ProviderSetup> = {
  claude: {
    installCommand: "curl -fsSL https://claude.ai/install.sh | bash",
    installSteps: [
      "Open Terminal and run the installer below.",
      "Open a new terminal window and run claude --version. When a version appears, check the installation here.",
    ],
    loginSteps: [
      "Run the sign-in command below in Terminal.",
      "Complete the browser sign-in with your Claude Pro, Max, Team or Enterprise account. For Anthropic Console billing, use claude auth login --console instead.",
      "Wait for Terminal to confirm sign-in, then check the connection here.",
    ],
    verifyCommand: "claude auth status",
    installDocs: "https://code.claude.com/docs/en/setup",
    loginDocs: "https://code.claude.com/docs/en/authentication",
  },
  codex: {
    installCommand: "curl -fsSL https://chatgpt.com/codex/install.sh | sh",
    installSteps: [
      "Open Terminal and run the installer below.",
      "Open a new terminal window and run codex --version. When a version appears, check the installation here.",
    ],
    loginSteps: [
      "Run the sign-in command below in Terminal.",
      "Sign in with ChatGPT in the browser window that opens. Return to Terminal and wait for confirmation.",
      "Check the connection here. If the browser callback fails, use codex login --device-auth and follow its link and one-time code.",
    ],
    verifyCommand: "codex login status",
    installDocs: "https://developers.openai.com/codex/cli",
    loginDocs: "https://developers.openai.com/codex/auth",
  },
  opencode: {
    installCommand: "curl -fsSL https://opencode.ai/install | bash",
    installSteps: [
      "Open Terminal and run the installer below.",
      "Open a new terminal window and run opencode --version. When a version appears, check the installation here.",
    ],
    loginSteps: [
      "Run the sign-in command below in Terminal.",
      "Select your provider and a sign-in method from the list. Complete its browser sign-in or paste its API key into Terminal when prompted.",
      "Run opencode auth list to confirm the provider is saved, then check the connection here. In OpenCode, use /models to select a model.",
    ],
    verifyCommand: "opencode auth list",
    installDocs: "https://opencode.ai/docs/",
    loginDocs: "https://opencode.ai/docs/cli/#auth",
  },
  gemini: {
    installCommand: "npm install -g @google/gemini-cli",
    installSteps: [
      "Install Node.js 20 or later, then run the command below in Terminal. With Homebrew, you can use brew install gemini-cli instead.",
      "Run gemini --version. When a version appears, check the installation here.",
    ],
    loginSteps: [
      "Run gemini in Terminal and select Sign in with Google.",
      "Complete the browser sign-in. For Google AI Pro or Ultra, use the Google account linked to your subscription.",
      "Return to Terminal, wait for Gemini to open, then check the connection here. Work or school accounts may also require a Google Cloud project; follow the official guide below.",
    ],
    installDocs: "https://geminicli.com/docs/get-started/installation/",
    loginDocs: "https://geminicli.com/docs/get-started/authentication/",
  },
};

/** Human names for services listed under an OpenCode account. */
export const PROVIDER_NAMES: Record<string, string> = {
  anthropic: "Anthropic",
  openai: "OpenAI",
  google: "Google",
  opencode: "OpenCode Zen",
  "opencode-go": "OpenCode Go",
  openrouter: "OpenRouter",
  github: "GitHub Copilot",
  "github-copilot": "GitHub Copilot",
};
