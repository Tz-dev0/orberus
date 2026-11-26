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
  /** Its place among your own rules, to remove it; null for a team rule. */
  index: number | null;
}

/** One way to say where a new rule applies. */
export interface RuleChoice {
  label: string;
  path: string | null;
  gitRemote: string | null;
}

/** What the "new rule" form offers once a folder is chosen. */
export interface RuleDraft {
  /** The chosen folder: "~/code/acme-api". */
  folder: string;
  /** Widest first, never empty. */
  choices: RuleChoice[];
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

/** One conversation of a profile. */
export interface Conversation {
  id: string;
  title: string;
  /** Where it worked and when it was last used. */
  detail: string;
  size: string;
  /** Changed in the last hour: it may still be open. */
  inUse: boolean;
}

export interface Conversations {
  /** The biggest ones, biggest first. */
  items: Conversation[];
  /** How many smaller ones are not listed. */
  more: number;
}

export interface StorageOverview {
  profiles: Storage[];
  /** All profiles together: "3.5 GB". */
  total: string;
}

/** One project a profile was used in. */
export interface UsageProject {
  folder: string;
  /** Its part of the profile's usage: "63%". */
  share: string;
  /** What today's rules say about it. */
  state: "covered" | "default" | "elsewhere" | "gone" | "unknown";
  /** The same in a few words; empty when a rule covers it. */
  note: string;
  /** The full explanation, shown on hover. */
  why: string;
  /** True when the line deserves a look. */
  flagged: boolean;
}

/** One conversation, by what it sends with each answer. */
export interface UsageTalk {
  id: string;
  title: string;
  detail: string;
  /** Tokens sent with each answer, on average: "417k". */
  context: string;
  heavy: boolean;
}

export interface Usage {
  name: string;
  /** Estimated weight, to draw the bars to scale. */
  weight: number;
  /** Its part of all profiles: "72%". Empty with no activity. */
  share: string;
  projects: UsageProject[];
  moreProjects: number;
  talks: UsageTalk[];
  /** Projects that deserve a look. */
  flagged: number;
  reread: string;
}

export interface UsageOverview {
  profiles: Usage[];
  days: number;
  /** Projects that deserve a look, all profiles together. */
  flagged: number;
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
  /** Which projects and conversations used each profile lately. Reads the
   *  conversation files, so it is slower than `overview`. */
  usage(): Promise<UsageOverview>;
  setDefault(name: string): Promise<void>;
  addProfile(name: string, reuseLogin: boolean): Promise<void>;
  /** Takes the profile and its own rules out of the config. Its Claude Code
   *  folder is kept, unless `purge` asks to delete it with its login. */
  removeProfile(name: string, purge: boolean): Promise<void>;
  /** Asks which folder a new rule is about. Resolves to null when the
   *  choice is cancelled. */
  pickRuleFolder(): Promise<RuleDraft | null>;
  addRule(choice: RuleChoice, profile: string, locked: boolean): Promise<void>;
  /** Removes one of your own rules, given as the page shows it. */
  removeRule(rule: Rule): Promise<void>;
  /** The biggest conversations of a profile. */
  conversations(name: string): Promise<Conversations>;
  /** Moves one conversation to the Trash. */
  trashConversation(name: string, id: string): Promise<void>;
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
  usage: () => invoke<UsageOverview>("usage"),
  setDefault: (name) => invoke("set_default", { name }),
  addProfile: (name, reuseLogin) => invoke("add_profile", { name, reuseLogin }),
  pickRuleFolder: () => invoke<RuleDraft | null>("pick_rule_folder"),
  addRule: (choice, profile, locked) =>
    invoke("add_rule", { path: choice.path, gitRemote: choice.gitRemote, profile, locked }),
  removeRule: (rule) =>
    invoke("remove_rule", {
      index: rule.index,
      path: rule.path,
      gitRemote: rule.gitRemote,
      profile: rule.profile,
      locked: rule.locked,
    }),
  conversations: (name) => invoke<Conversations>("conversations", { name }),
  trashConversation: (name, id) => invoke("trash_conversation", { name, id }),
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
  const talks: Record<string, Conversation[]> = {
    perso: [
      { id: "a1", title: "Check the invoices export", detail: "~/Desktop, 2 days ago", size: "815 MB", inUse: false },
      { id: "a2", title: "Mobile career and recognition", detail: "~/Desktop, 1 day ago", size: "779 MB", inUse: false },
      { id: "a3", title: "Check the invoices export", detail: "~/Desktop, 20 minutes ago", size: "581 MB", inUse: true },
      { id: "a4", title: "Untitled (9879d850)", detail: "~/code/app, 13 days ago", size: "37 MB", inUse: false },
    ],
    work: [
      { id: "b1", title: "Payment retries", detail: "~/clients/acme, 4 days ago", size: "600 MB", inUse: false },
    ],
  };
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
      { path: null, gitRemote: "github.com/acme/*", profile: "work", locked: true, source: "team.toml", index: null },
      { path: "~/clients/acme", gitRemote: null, profile: "work", locked: false, source: "config.toml", index: 0 },
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
    usage: async () => {
      const sample: Record<string, Omit<Usage, "name">> = {
        perso: {
          weight: 28,
          share: "28%",
          projects: [
            { folder: "~/code/blog", share: "79%", state: "default", note: "no rule", why: "No rule names this project: it uses perso only as the default", flagged: true },
            { folder: "~/code/dotfiles", share: "21%", state: "covered", note: "", why: "Sent to perso by pinned with .claude-profile", flagged: false },
          ],
          moreProjects: 0,
          talks: [
            { id: "c1", title: "Blog redesign", detail: "~/code/blog, 74% of this profile", context: "142k", heavy: false },
            { id: "c2", title: "Untitled (e5f6a7b8)", detail: "~/code/dotfiles, 21% of this profile", context: "38k", heavy: false },
          ],
          flagged: 1,
          reread: "About 81% went to reading context again",
        },
        work: {
          weight: 72,
          share: "72%",
          projects: [
            { folder: "~/code/acme-api", share: "63%", state: "covered", note: "", why: 'Sent to work by locked team rule git_remote = "github.com/acme/*"', flagged: false },
            { folder: "~/code/blog", share: "26%", state: "elsewhere", note: "would use perso", why: "Started there today, claude would use perso (default profile). It ran with work by an explicit choice, or the rules changed since", flagged: true },
            { folder: "~/code/acme-web", share: "9%", state: "covered", note: "", why: 'Sent to work by locked team rule git_remote = "github.com/acme/*"', flagged: false },
            { folder: "~/code/old-prototype", share: "2%", state: "gone", note: "folder gone", why: "The folder was deleted, so its rules cannot be checked", flagged: false },
          ],
          moreProjects: 3,
          talks: [
            { id: "d1", title: "Payment retries", detail: "~/code/acme-api, 61% of this profile", context: "417k", heavy: true },
            { id: "d2", title: "Migrate the invoices table", detail: "~/code/acme-api, 20% of this profile", context: "229k", heavy: true },
            { id: "d3", title: "Fix flaky login test", detail: "~/code/acme-web, 3% of this profile", context: "64k", heavy: false },
          ],
          flagged: 1,
          reread: "About 88% went to reading context again",
        },
      };
      const idle = { weight: 0, share: "", projects: [], moreProjects: 0, talks: [], flagged: 0, reread: "" };
      const profiles = state.profiles.map(({ name }) => ({ name, ...(sample[name] ?? idle) }));
      return { profiles, days: 7, flagged: profiles.reduce((sum, profile) => sum + profile.flagged, 0) };
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
    pickRuleFolder: async () => ({
      folder: "~/code/blog",
      choices: [
        { label: "Every repository of github.com/me", path: null, gitRemote: "github.com/me/*" },
        { label: "Only the repository github.com/me/blog", path: null, gitRemote: "github.com/me/blog" },
        { label: "The folder ~/code/blog and everything below", path: "~/code/blog", gitRemote: null },
      ],
    }),
    addRule: async (choice, profile, locked) => {
      if (!state.profiles.some((known) => known.name === profile)) throw `unknown profile '${profile}'`;
      const same = (rule: Rule) =>
        rule.source === "config.toml" &&
        rule.path === choice.path &&
        rule.gitRemote === choice.gitRemote &&
        rule.profile === profile &&
        rule.locked === locked;
      if (state.rules.some(same)) throw "this rule already exists";
      const own = state.rules.filter((rule) => rule.index !== null).length;
      state.rules.push({ path: choice.path, gitRemote: choice.gitRemote, profile, locked, source: "config.toml", index: own });
      const target = state.profiles.find((known) => known.name === profile);
      if (target) target.rules += 1;
    },
    removeRule: async (gone) => {
      state.rules = state.rules.filter((rule) => rule.index !== gone.index);
      // Your own rules are numbered again from 0.
      let next = 0;
      for (const rule of state.rules) if (rule.index !== null) rule.index = next++;
    },
    conversations: async (name) => ({
      items: structuredClone(talks[name] ?? []),
      more: name === "perso" ? 54 : 0,
    }),
    trashConversation: async (name, id) => {
      talks[name] = (talks[name] ?? []).filter((talk) => talk.id !== id);
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
