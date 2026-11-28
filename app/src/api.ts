// The calls to the Rust side, with their types. The shapes mirror the
// structs in src-tauri/src/data.rs.
//
// What comes back is data, never a sentence: sizes in bytes, ages in
// seconds, shares from 0 to 1, states as short codes. The words are in
// i18n.ts. An error is the exception: its text comes from Rust, in English.

import { invoke } from "@tauri-apps/api/core";

export interface Profile {
  name: string;
  /** null when nobody is signed in to this profile. */
  account: string | null;
  /** The account's subscription, as a short name ("Max 5x"); null when unknown. */
  plan: string | null;
  /** Its config folder: "~/.claude". */
  location: string;
  /** True when that folder is Claude Code's own default one. */
  native: boolean;
  isDefault: boolean;
  rules: number;
  /** How many of those rules come from team.toml: they stay when the
   *  profile is removed. */
  teamRules: number;
  /** False when its folder must never be deleted, as for ~/.claude. */
  canPurge: boolean;
  /** How long it keeps its conversations, in days; null when its settings
   *  file cannot be read. */
  keepsDays: number | null;
  /** Why that file cannot be read, when it cannot. */
  keepsError: string | null;
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
  /** Everything, in bytes. */
  total: number;
  /** What takes it, in bytes. */
  conversations: number;
  fileHistory: number;
  extensions: number;
  other: number;
}

/** One conversation of a profile. */
export interface Conversation {
  id: string;
  /** null when it has no title. */
  title: string | null;
  /** Where it worked: "~/app". */
  folder: string | null;
  /** How long ago it was last used, in seconds. */
  idleSecs: number;
  /** In bytes. */
  size: number;
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
  /** All profiles together, in bytes. */
  total: number;
}

/** Why a folder gets its profile. */
export interface Reason {
  kind: "locked" | "explicit" | "file" | "rule" | "default";
  /** With "locked" and "rule": the rule as written in the config. */
  rule: string | null;
  /** With "locked" and "rule": "team.toml" or "config.toml". */
  source: string | null;
  /** With "file": the .orberus-profile that decides. */
  file: string | null;
}

/** One project a profile was used in. */
export interface UsageProject {
  folder: string;
  /** Its part of the profile's usage, from 0 to 1. */
  share: number;
  /** What today's rules say about it. */
  state: "covered" | "default" | "elsewhere" | "gone" | "unknown";
  /** With "elsewhere": the profile claude would use there today. */
  profile: string | null;
  /** With "covered" and "elsewhere": what decides. */
  reason: Reason | null;
  /** With "unknown": why the rules could not be checked. */
  error: string | null;
  /** True when the line deserves a look. */
  flagged: boolean;
}

/** One conversation, by what it sends with each answer. */
export interface UsageTalk {
  id: string;
  title: string | null;
  /** Where it worked. */
  folder: string;
  /** Its part of the profile's usage, from 0 to 1. */
  share: number;
  /** Tokens sent with each answer, on average. */
  context: number;
  heavy: boolean;
}

export interface Usage {
  name: string;
  /** Estimated weight, to draw the bars to scale. */
  weight: number;
  /** Its part of all profiles, from 0 to 1; null with no activity. */
  share: number | null;
  projects: UsageProject[];
  moreProjects: number;
  talks: UsageTalk[];
  /** Projects that deserve a look. */
  flagged: number;
  /** The part that went to reading context again, from 0 to 1; null with
   *  no activity. */
  reread: number | null;
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
  /** The terminal app chosen to open Claude Code in; null leaves it to
   *  macOS. */
  terminal: string | null;
  /** True when macOS opens the app at login. */
  openAtLogin: boolean;
}

/** One limit of an account, as it stands now. */
export interface LimitWindow {
  /** From 0 to 100. */
  used: number;
  /** Seconds until it starts over, when that is known. */
  resetsIn: number | null;
  /** True when it started over since the numbers were noted (used is 0). */
  startedOver: boolean;
}

/** The 5-hour and weekly limits of one profile's account: the last numbers
 *  Claude Code or Claude Desktop noted, not a live reading. */
export interface Limits {
  name: string;
  /** Whether Claude Code reports them to Orberus for this profile. */
  state: "on" | "off" | "unknown";
  fiveHour: LimitWindow | null;
  sevenDay: LimitWindow | null;
  /** How old the numbers are, in seconds; null when there are none. */
  ageSecs: number | null;
  /** Who noted them: Claude Code (its status line) or Claude Desktop (the
   *  file it keeps); null when there are none. */
  source: "code" | "desktop" | null;
}

/** A terminal app installed on this Mac that can run a script file. */
export interface Terminal {
  name: string;
  path: string;
  selected: boolean;
}

/** An editor of the VS Code family found on this Mac. */
export interface Editor {
  name: string;
  /** Its settings file: what `setEditor` takes. */
  settings: string;
  settingsShort: string;
  /** on: its Claude Code extension goes through Orberus. off: it starts
   *  Claude Code itself. absent: the extension is not installed there.
   *  other: it goes through another program. unreadable: the settings file
   *  is not understood. */
  state: "on" | "off" | "absent" | "other" | "unreadable";
  /** The other program, or why the file is not understood. */
  detail: string | null;
}

export interface Api {
  overview(): Promise<Overview>;
  /** Measures every profile's folder. Slower than `overview`. */
  storage(): Promise<StorageOverview>;
  /** Which projects and conversations used each profile lately. Reads the
   *  conversation files, so it is slower than `overview`. */
  usage(): Promise<UsageOverview>;
  /** The limits Claude Code last reported for each profile. Fast. */
  limits(): Promise<Limits[]>;
  /** Turns that reporting on or off: adds or removes a status line in the
   *  profile's Claude Code settings. */
  setLimits(name: string, on: boolean): Promise<void>;
  setDefault(name: string): Promise<void>;
  addProfile(name: string, reuseLogin: boolean): Promise<void>;
  /** Takes the profile and its own rules out of the config. Its Claude Code
   *  folder is kept, unless `purge` asks to delete it with its login. */
  removeProfile(name: string, purge: boolean): Promise<void>;
  /** Asks which folder a new rule is about. Resolves to null when the
   *  choice is cancelled. */
  pickRuleFolder(title: string): Promise<RuleDraft | null>;
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
  openClaude(name: string, title: string): Promise<boolean>;
  /** Names the "Quit" line of the menu under a right click on the icon. */
  setQuitLabel(label: string): Promise<void>;
  /** The terminal apps that can be chosen. */
  terminals(): Promise<Terminal[]>;
  /** Chooses the terminal app by its path; null goes back to macOS's own. */
  setTerminal(path: string | null): Promise<void>;
  /** The editors whose Claude Code extension can go through Orberus. */
  editors(): Promise<Editor[]>;
  /** Sends an editor's extension through Orberus, or back. */
  setEditor(settings: string, on: boolean): Promise<void>;
  /** The version to update to, or null. GitHub is asked at most once a day. */
  update(): Promise<string | null>;
  /** Opens the page that says how to update. */
  openUpdatePage(): Promise<void>;
  /** Opens the app when the user logs in, or stops doing so. */
  setOpenAtLogin(on: boolean): Promise<void>;
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
  limits: () => invoke<Limits[]>("limits"),
  setLimits: (name, on) => invoke("set_limits", { name, on }),
  addProfile: (name, reuseLogin) => invoke("add_profile", { name, reuseLogin }),
  pickRuleFolder: (title) => invoke<RuleDraft | null>("pick_rule_folder", { title }),
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
  openClaude: (name, title) => invoke<boolean>("open_claude", { name, title }),
  setQuitLabel: (label) => invoke("set_quit_label", { label }),
  terminals: () => invoke<Terminal[]>("terminals"),
  setTerminal: (path) => invoke("set_terminal", { path }),
  editors: () => invoke<Editor[]>("editors"),
  setEditor: (settings, on) => invoke("set_editor", { settings, on }),
  setOpenAtLogin: (on) => invoke("set_open_at_login", { on }),
  update: () => invoke<string | null>("update"),
  openUpdatePage: () => invoke("open_update_page"),
  openConfig: () => invoke("open_config"),
  fit: (height) => invoke("fit", { height }),
  hide: () => invoke("hide"),
  quit: () => invoke("quit"),
};

// Sample data, used only when the page is opened in a plain browser with
// `npm run dev`. It makes the layout easy to work on without the app shell.
function preview(): Api {
  const day = 86_400;
  const talks: Record<string, Conversation[]> = {
    perso: [
      { id: "a1", title: "Check the invoices export", folder: "~/Desktop", idleSecs: 2 * day, size: 815e6, inUse: false },
      { id: "a2", title: "Mobile career and recognition", folder: "~/Desktop", idleSecs: day, size: 779e6, inUse: false },
      { id: "a3", title: "Check the invoices export", folder: "~/Desktop", idleSecs: 20 * 60, size: 581e6, inUse: true },
      { id: "9879d850-0000", title: null, folder: "~/code/app", idleSecs: 13 * day, size: 37e6, inUse: false },
    ],
    work: [{ id: "b1", title: "Payment retries", folder: "~/clients/acme", idleSecs: 4 * day, size: 600e6, inUse: false }],
  };
  const editor = (name: string, state: Editor["state"], detail: string | null = null): Editor => ({
    name,
    state,
    detail,
    settings: `/Users/you/Library/Application Support/${name}/User/settings.json`,
    settingsShort: `~/Library/Application Support/${name}/User/settings.json`,
  });
  const editorSample: Editor[] = [
    editor("Cursor", "absent"),
    editor("VS Code", "off"),
    editor("Windsurf", "other", "/opt/acme/claude-wrap"),
  ];
  const state: Overview = {
    shimInstalled: true,
    canReuseLogin: false,
    terminal: null,
    // On by default, as in the real app.
    openAtLogin: true,
    profiles: [
      { name: "perso", account: "me@example.com", plan: "Max 5x", location: "~/.claude", native: true, isDefault: true, rules: 0, teamRules: 0, canPurge: false, keepsDays: 120, keepsError: null },
      {
        name: "work",
        account: "me@acme.example",
        plan: "Team",
        location: "~/.orberus/profiles/work",
        native: false,
        isDefault: false,
        rules: 2,
        teamRules: 1,
        canPurge: true,
        keepsDays: 30,
        keepsError: null,
      },
      {
        name: "client",
        account: null,
        plan: null,
        location: "~/.orberus/profiles/client",
        native: false,
        isDefault: false,
        rules: 0,
        teamRules: 0,
        canPurge: true,
        keepsDays: 30,
        keepsError: null,
      },
    ],
    rules: [
      { path: null, gitRemote: "github.com/acme/*", profile: "work", locked: true, source: "team.toml", index: null },
      { path: "~/clients/acme", gitRemote: null, profile: "work", locked: false, source: "config.toml", index: 0 },
    ],
  };
  const noLimits = { fiveHour: null, sevenDay: null, ageSecs: null, source: null };
  // perso: Claude Desktop is logged in to that account and spoke last, with
  // the reset times Claude Code gave earlier. work: Claude Code only.
  const limitSample: Record<string, Omit<Limits, "name">> = {
    perso: {
      state: "off",
      fiveHour: { used: 16, resetsIn: 3 * 3600 + 300, startedOver: false },
      sevenDay: { used: 13, resetsIn: null, startedOver: false },
      ageSecs: 190,
      source: "desktop",
    },
    work: {
      state: "on",
      fiveHour: { used: 78, resetsIn: 40 * 60, startedOver: false },
      sevenDay: { used: 93, resetsIn: 86_400 + 7200, startedOver: false },
      ageSecs: 3 * 3600,
      source: "code",
    },
  };
  return {
    overview: async () => structuredClone(state),
    storage: async () => {
      // conversations, file history, plugins and skills, other: in bytes.
      const sample: Record<string, [number, number, number, number]> = {
        perso: [3.4e9, 35e6, 41e6, 24e6],
        work: [600e6, 0, 31e6, 9e6],
      };
      const profiles = state.profiles.map(({ name }) => {
        const [conversations, fileHistory, extensions, other] = sample[name] ?? [0, 0, 0, 0];
        return { name, total: conversations + fileHistory + extensions + other, conversations, fileHistory, extensions, other };
      });
      return { profiles, total: profiles.reduce((sum, profile) => sum + profile.total, 0) };
    },
    usage: async () => {
      const team: Reason = { kind: "locked", rule: 'git_remote = "github.com/acme/*"', source: "team.toml", file: null };
      const byDefault: Reason = { kind: "default", rule: null, source: null, file: null };
      const pinned: Reason = { kind: "file", rule: null, source: null, file: "~/code/dotfiles/.orberus-profile" };
      const project = (folder: string, share: number, state: UsageProject["state"], more: Partial<UsageProject> = {}): UsageProject => ({
        folder,
        share,
        state,
        profile: null,
        reason: null,
        error: null,
        flagged: false,
        ...more,
      });
      const sample: Record<string, Omit<Usage, "name">> = {
        perso: {
          weight: 28,
          share: 0.28,
          projects: [
            project("~/code/blog", 0.79, "default", { flagged: true }),
            project("~/code/dotfiles", 0.21, "covered", { reason: pinned }),
          ],
          moreProjects: 0,
          talks: [
            { id: "c1", title: "Blog redesign", folder: "~/code/blog", share: 0.74, context: 142_000, heavy: false },
            { id: "e5f6a7b8-0000", title: null, folder: "~/code/dotfiles", share: 0.21, context: 38_000, heavy: false },
          ],
          flagged: 1,
          reread: 0.81,
        },
        work: {
          weight: 72,
          share: 0.72,
          projects: [
            project("~/code/acme-api", 0.63, "covered", { reason: team }),
            project("~/code/blog", 0.26, "elsewhere", { profile: "perso", reason: byDefault, flagged: true }),
            project("~/code/acme-web", 0.09, "covered", { reason: team }),
            project("~/code/old-prototype", 0.02, "gone"),
          ],
          moreProjects: 3,
          talks: [
            { id: "d1", title: "Payment retries", folder: "~/code/acme-api", share: 0.61, context: 417_000, heavy: true },
            { id: "d2", title: "Migrate the invoices table", folder: "~/code/acme-api", share: 0.2, context: 229_000, heavy: true },
            { id: "d3", title: "Fix flaky login test", folder: "~/code/acme-web", share: 0.03, context: 64_000, heavy: false },
          ],
          flagged: 1,
          reread: 0.88,
        },
      };
      const idle = { weight: 0, share: null, projects: [], moreProjects: 0, talks: [], flagged: 0, reread: null };
      const profiles = state.profiles.map(({ name }) => ({ name, ...(sample[name] ?? idle) }));
      return { profiles, days: 7, flagged: profiles.reduce((sum, profile) => sum + profile.flagged, 0) };
    },
    limits: async () =>
      state.profiles.map(({ name }) => ({ name, ...(limitSample[name] ?? { state: "off" as const, ...noLimits }) })),
    setLimits: async (name, on) => {
      // Like the real thing: the numbers already noted stay.
      limitSample[name] = { ...noLimits, ...limitSample[name], state: on ? "on" : "off" };
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
        plan: reuseLogin ? "Max 5x" : null,
        location: reuseLogin ? "~/.claude" : `~/.orberus/profiles/${name}`,
        native: reuseLogin,
        isDefault: state.profiles.length === 0,
        rules: 0,
        teamRules: 0,
        canPurge: !reuseLogin,
        keepsDays: 30,
        keepsError: null,
      });
      state.profiles.sort((a, b) => a.name.localeCompare(b.name));
      if (reuseLogin) state.canReuseLogin = false;
    },
    pickRuleFolder: async () => ({
      folder: "~/code/blog",
      choices: [
        { path: null, gitRemote: "github.com/me/*" },
        { path: null, gitRemote: "github.com/me/blog" },
        { path: "~/code/blog", gitRemote: null },
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
      if (!Number.isInteger(days) || days < 1) {
        throw "Claude Code refuses 0: the shortest it accepts is 1 day";
      }
      profile.keepsDays = days;
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
    terminals: async () =>
      ["Ghostty", "Terminal"].map((name) => ({
        name,
        path: `/Applications/${name}.app`,
        selected: state.terminal === name,
      })),
    setTerminal: async (path) => {
      state.terminal = path ? path.replace(/^.*\/|\.app$/g, "") : null;
    },
    editors: async () => editorSample.map((editor) => ({ ...editor })),
    setEditor: async (settings, on) => {
      const editor = editorSample.find((found) => found.settings === settings);
      if (editor) editor.state = on ? "on" : "off";
    },
    setOpenAtLogin: async (on) => {
      state.openAtLogin = on;
    },
    // No notice in the preview, unless asked for: `?update` in the address.
    update: async () => (new URLSearchParams(location.search).has("update") ? "0.2.0" : null),
    openUpdatePage: async () => {},
    setQuitLabel: async () => {},
    openConfig: async () => {},
    fit: async () => {},
    hide: async () => {},
    quit: async () => {},
  };
}

// Tauri injects this object into the page it hosts.
export const insideApp = "__TAURI_INTERNALS__" in window;

export const api: Api = insideApp ? tauri : preview();
