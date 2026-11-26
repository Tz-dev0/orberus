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
  /** How long it keeps its conversations, in words: "30 days". */
  keeps: string;
  /** The same in days; null when its settings file cannot be read. */
  keepsDays: number | null;
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

/** The disk space one profile takes. */
export interface Storage {
  name: string;
  /** In bytes, to draw the bars to scale. */
  total: number;
  /** For people: "3.4 GB". */
  size: string;
  /** What takes it, in words. */
  detail: string;
}

export interface StorageOverview {
  profiles: Storage[];
  /** All profiles together: "3.5 GB". */
  total: string;
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
  /** Measures every profile's folder. Slower than `overview`. */
  storage(): Promise<StorageOverview>;
  setDefault(name: string): Promise<void>;
  addProfile(name: string, reuseLogin: boolean): Promise<void>;
  /** Takes the profile and its own rules out of the config. Its Claude Code
   *  folder is kept, unless `purge` asks to delete it with its login. */
  removeProfile(name: string, purge: boolean): Promise<void>;
  /** Sets how many days the profile keeps its conversations. */
  setRetention(name: string, days: number): Promise<void>;
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
  storage: () => invoke<StorageOverview>("storage"),
  setDefault: (name) => invoke("set_default", { name }),
  addProfile: (name, reuseLogin) => invoke("add_profile", { name, reuseLogin }),
  setRetention: (name, days) => invoke("set_retention", { name, days }),
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
      { name: "perso", account: "me@example.com", location: "~/.claude", isDefault: true, rules: 0, teamRules: 0, canPurge: false, keeps: "120 days", keepsDays: 120 },
      {
        name: "work",
        account: "me@acme.example",
        location: "~/.claude-profiles/profiles/work",
        isDefault: false,
        rules: 2,
        teamRules: 1,
        canPurge: true,
        keeps: "30 days",
        keepsDays: 30,
      },
      {
        name: "client",
        account: null,
        location: "~/.claude-profiles/profiles/client",
        isDefault: false,
        rules: 0,
        teamRules: 0,
        canPurge: true,
        keeps: "30 days",
        keepsDays: 30,
      },
    ],
    rules: [
      { path: null, gitRemote: "github.com/acme/*", profile: "work", locked: true, source: "team.toml" },
      { path: "~/clients/acme", gitRemote: null, profile: "work", locked: false, source: "config.toml" },
    ],
  };
  return {
    overview: async () => structuredClone(state),
    storage: async () => {
      const sample: Record<string, [number, string, string]> = {
        perso: [3.5e9, "3.5 GB", "3.4 GB conversations, 41 MB plugins and skills, 35 MB file history"],
        work: [6.4e8, "640 MB", "600 MB conversations, 31 MB plugins and skills, 9.0 MB other"],
      };
      const profiles = state.profiles.map(({ name }) => {
        const [total, size, detail] = sample[name] ?? [0, "0 B", ""];
        return { name, total, size, detail };
      });
      return { profiles, total: "4.1 GB" };
    },
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
        keeps: "30 days",
        keepsDays: 30,
      });
      state.profiles.sort((a, b) => a.name.localeCompare(b.name));
      if (reuseLogin) state.canReuseLogin = false;
    },
    setRetention: async (name, days) => {
      const profile = state.profiles.find((profile) => profile.name === name);
      if (!profile) throw `unknown profile '${name}'`;
      profile.keepsDays = days;
      if (!Number.isInteger(days) || days < 1) {
        throw "Claude Code refuses 0: the shortest it accepts is 1 day";
      }
      profile.keeps = days === 365 ? "1 year" : days === 1 ? "1 day" : `${days} days`;
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
