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
  /** How many of those rules come from team.toml: they stay when the
   *  profile is removed. */
  teamRules: number;
  /** False when its folder must never be deleted, as for ~/.claude. */
  canPurge: boolean;
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
  /** True while no profile reuses ~/.claude: a new one may take it. */
  canReuseLogin: boolean;
}

export interface Api {
  overview(): Promise<Overview>;
  setDefault(name: string): Promise<void>;
  addProfile(name: string, reuseLogin: boolean): Promise<void>;
  /** Takes the profile and its own rules out of the config. Its Claude Code
   *  folder is kept, unless `purge` asks to delete it with its login. */
  removeProfile(name: string, purge: boolean): Promise<void>;
  /** Asks for a project folder, then opens Claude Code there in a terminal.
   *  Resolves to false when the folder choice is cancelled. */
  openClaude(name: string): Promise<boolean>;
  openConfig(): Promise<void>;
  /** Resizes the window to the height of its content. */
  fit(height: number): Promise<void>;
  hide(): Promise<void>;
  quit(): Promise<void>;
}

const tauri: Api = {
  overview: () => invoke<Overview>("overview"),
  setDefault: (name) => invoke("set_default", { name }),
  addProfile: (name, reuseLogin) => invoke("add_profile", { name, reuseLogin }),
  removeProfile: (name, purge) => invoke("remove_profile", { name, purge }),
  openClaude: (name) => invoke<boolean>("open_claude", { name }),
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
    canReuseLogin: false,
    profiles: [
      { name: "perso", account: "me@example.com", location: "~/.claude", isDefault: true, rules: 0, teamRules: 0, canPurge: false },
      {
        name: "work",
        account: "me@acme.example",
        location: "~/.claude-profiles/profiles/work",
        isDefault: false,
        rules: 2,
        teamRules: 1,
        canPurge: true,
      },
      {
        name: "client",
        account: null,
        location: "~/.claude-profiles/profiles/client",
        isDefault: false,
        rules: 0,
        teamRules: 0,
        canPurge: true,
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
    // The same answers as the Rust side, so the form can be tried in a browser.
    addProfile: async (name, reuseLogin) => {
      name = name.trim();
      if (!/^[A-Za-z0-9_-]+$/.test(name)) {
        throw `invalid profile name '${name}': use letters, digits, '-' and '_'`;
      }
      if (state.profiles.some((profile) => profile.name === name)) {
        throw `profile '${name}' already exists`;
      }
      state.profiles.push({
        name,
        account: reuseLogin ? "me@example.com" : null,
        location: reuseLogin ? "~/.claude" : `~/.claude-profiles/profiles/${name}`,
        isDefault: state.profiles.length === 0,
        rules: 0,
        teamRules: 0,
        canPurge: !reuseLogin,
      });
      state.profiles.sort((a, b) => a.name.localeCompare(b.name));
      if (reuseLogin) state.canReuseLogin = false;
    },
    removeProfile: async (name, _purge) => {
      state.profiles = state.profiles.filter((profile) => profile.name !== name);
      // Team rules cannot be removed from here.
      state.rules = state.rules.filter(
        (rule) => rule.profile !== name || rule.source === "team.toml",
      );
    },
    openClaude: async (name) => {
      if (name === "client") throw "Preview only: a browser cannot open a terminal.";
      return true;
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
