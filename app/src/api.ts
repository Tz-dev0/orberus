// The calls to the Rust side, with their types. The shapes mirror the
// structs in src-tauri/src/data.rs.

import { invoke } from "@tauri-apps/api/core";

export interface Profile {
  name: string;
  /** null when nobody is signed in to this profile. */
  account: string | null;
  location: string;
  isDefault: boolean;
  rules: number;
}

export interface Rule {
  /** Folder the rule covers, when it has one. */
  path: string | null;
  /** Git remote pattern, when it has one. */
  gitRemote: string | null;
  profile: string;
  locked: boolean;
  /** "team.toml" or "config.toml". */
  source: string;
}

export interface Overview {
  profiles: Profile[];
  rules: Rule[];
  shimInstalled: boolean;
}

export interface Api {
  overview(): Promise<Overview>;
  setDefault(name: string): Promise<void>;
  openClaude(name: string): Promise<void>;
  openConfig(): Promise<void>;
  /** Resizes the window to the height of its content. */
  fit(height: number): Promise<void>;
  hide(): Promise<void>;
  quit(): Promise<void>;
}

const tauri: Api = {
  overview: () => invoke<Overview>("overview"),
  setDefault: (name) => invoke("set_default", { name }),
  openClaude: (name) => invoke("open_claude", { name }),
  openConfig: () => invoke("open_config"),
  fit: (height) => invoke("fit", { height }),
  hide: () => invoke("hide"),
  quit: () => invoke("quit"),
};

// Sample data, used only when the page is opened in a plain browser with
// `npm run dev`. It makes the layout easy to work on without the app shell.
function preview(): Api {
  const state: Overview = {
    shimInstalled: true,
    profiles: [
      { name: "perso", account: "me@example.com", location: "~/.claude", isDefault: true, rules: 0 },
      {
        name: "work",
        account: "me@acme.example",
        location: "~/.claude-profiles/profiles/work",
        isDefault: false,
        rules: 2,
      },
      {
        name: "client",
        account: null,
        location: "~/.claude-profiles/profiles/client",
        isDefault: false,
        rules: 0,
      },
    ],
    rules: [
      { path: null, gitRemote: "github.com/acme/*", profile: "work", locked: true, source: "team.toml" },
      { path: "~/clients/acme", gitRemote: null, profile: "work", locked: false, source: "config.toml" },
    ],
  };
  return {
    overview: async () => structuredClone(state),
    setDefault: async (name) => {
      for (const profile of state.profiles) profile.isDefault = profile.name === name;
    },
    openClaude: async (name) => {
      if (name === "client") throw "Preview only: a browser cannot open a terminal.";
    },
    openConfig: async () => {},
    fit: async () => {},
    hide: async () => {},
    quit: async () => {},
  };
}

// Tauri injects this object into the page it hosts.
export const insideApp = "__TAURI_INTERNALS__" in window;

export const api: Api = insideApp ? tauri : preview();
