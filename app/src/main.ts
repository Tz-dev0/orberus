// The popover under the menu bar icon: profiles, rules, and a few actions.
// Plain TypeScript and DOM calls: the page is one small list, a framework
// would add more code than it removes.

import {
  api,
  insideApp,
  type Overview,
  type Profile,
  type Reason,
  type UsageProject,
  type Conversations,
  type Editor,
  type Limits,
  type LimitWindow,
  type Rule,
  type RuleDraft,
  type StorageOverview,
  type Terminal,
  type Usage,
  type UsageOverview,
} from "./api";
import { ago, bytes, count, days, language, percent, t, tokens, until } from "./i18n";

const root = document.querySelector<HTMLDivElement>("#app")!;
// In the app the window itself is the frosted panel. In a browser there is
// no window behind the page, so the stylesheet draws a stand-in.
document.documentElement.classList.toggle("in-browser", !insideApp);

/** Builds an element. Text goes through `textContent`, never `innerHTML`,
 *  so a profile name or an e-mail can never be read as markup. */
function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className = "",
  text = "",
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text) node.textContent = text;
  return node;
}

/** Names an element across rebuilds of the page. The page is built anew on
 *  every change, so this name is how `settle` tells what just appeared (and
 *  plays its entrance) from what was already on screen (and stays still). */
function keyed<T extends Element>(node: T, key: string): T {
  node.setAttribute("data-key", key);
  return node;
}

// Small line icons, drawn on a 16px grid.
const ICONS = {
  folder: "M2 4.5a1 1 0 0 1 1-1h3.2l1.3 1.5H13a1 1 0 0 1 1 1V12a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z",
  branch:
    "M5 3.2v9.6M5 5.4a1.6 1.6 0 1 0 0-3.2 1.6 1.6 0 0 0 0 3.2zm0 8.4a1.6 1.6 0 1 0 0-3.2 1.6 1.6 0 0 0 0 3.2zm6-6.2a1.6 1.6 0 1 0 0-3.2 1.6 1.6 0 0 0 0 3.2zm0 0c0 2.2-2.6 2.6-6 3",
  lock: "M4.5 7.2V5.4a3.5 3.5 0 0 1 7 0v1.8M3.6 7.2h8.8v6H3.6z",
  chevron: "M6 3.5 10.5 8 6 12.5",
  plus: "M8 3.5v9M3.5 8h9",
  bin: "M3 4.5h10M6.5 4.5V3h3v1.5M4.3 4.5l.6 8.5h6.2l.6-8.5M6.8 7v3.5M9.2 7v3.5",
} as const;

function icon(name: keyof typeof ICONS, label = ""): SVGElement {
  const space = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(space, "svg");
  svg.setAttribute("viewBox", "0 0 16 16");
  svg.setAttribute("class", `icon icon-${name}`);
  if (label) {
    svg.setAttribute("role", "img");
    svg.setAttribute("aria-label", label);
  } else {
    svg.setAttribute("aria-hidden", "true");
  }
  const path = document.createElementNS(space, "path");
  path.setAttribute("d", ICONS[name]);
  svg.append(path);
  return svg;
}

// Each profile has one color everywhere it appears, so "work" is recognized
// at a glance in the list and in the rules.
const HUES = ["#d99a3d", "#2fa39b", "#6f83e6", "#d6648b", "#63a852", "#9a78d8"];
const NO_PROFILE = "#8e8e93";
let hues = new Map<string, string>();

/** The color a name asks for. It depends on the name alone, so a profile
 *  keeps its color when others are added or removed. */
function preferredHue(name: string): number {
  let hash = 5381;
  for (const char of name) hash = (hash * 33 + (char.codePointAt(0) ?? 0)) >>> 0;
  return hash % HUES.length;
}

/** The first color still free, starting from the one the name asks for. */
function freeHue(name: string, taken: Set<number>): number {
  const start = preferredHue(name);
  for (let step = 0; step < HUES.length; step++) {
    const index = (start + step) % HUES.length;
    if (!taken.has(index)) return index;
  }
  return start; // more profiles than colors: sharing is unavoidable
}

function assignHues(profiles: Profile[]): number[] {
  const taken = new Set<number>();
  hues = new Map();
  for (const profile of profiles) {
    const index = freeHue(profile.name, taken);
    taken.add(index);
    hues.set(profile.name, HUES[index]);
  }
  return [...taken];
}

/** A rule can name a profile that no longer exists: it gets a neutral grey. */
function hue(name: string): string {
  return hues.get(name) ?? NO_PROFILE;
}

/** Runs a button's action. The button is disabled meanwhile, so a double
 *  click cannot open two terminals. */
function onClick(node: HTMLButtonElement, action: () => Promise<void>): void {
  node.type = "button";
  node.addEventListener("click", async () => {
    node.disabled = true;
    // Shown while the action runs: a profile's seal pulses until the
    // folder picker has answered.
    node.classList.add("busy");
    pending += 1;
    errorText = "";
    try {
      await action();
    } catch (error) {
      errorText = String(error);
      errorAt = Date.now();
    } finally {
      pending -= 1;
      node.disabled = false;
    }
    await refresh();
  });
}

// How many actions are running. A folder picker takes the focus away
// without the window being closed: see the "blur" listener.
let pending = 0;

let errorText = "";
let errorAt = 0;

// The "new profile" form. Its state lives here and not in the DOM, because
// the page is rebuilt each time the window comes back to the front.
let adding = false;
let draftName = "";
let draftReuse = false;
let formError = "";

function closeForm(): void {
  adding = false;
  draftName = "";
  draftReuse = false;
  formError = "";
}

// The profile whose removal is being confirmed, if any.
let removing: string | null = null;
// Whether its folder and login go too. Off each time the question opens.
let purging = false;

/** Takes the place of a profile's row while its removal is confirmed. */
function removeRow(profile: Profile, overview: Overview): HTMLElement {
  const row = keyed(el("li", "profile-form"), `confirm:${profile.name}`);
  row.style.setProperty("--hue", hue(profile.name));
  if (!profile.account) row.classList.add("signed-out");

  const line = el("div", "form-line");
  line.append(
    el("span", "seal", profile.name.slice(0, 1).toUpperCase()),
    el("span", "name", t("removeAsk", { name: profile.name })),
  );
  row.append(line);

  // Say exactly what goes and what stays, before the click.
  const own = profile.rules - profile.teamRules;
  const rules = own > 0 ? ` ${count(own, "ownRuleOne", "ownRuleMany")}` : "";
  const kept = profile.native ? t("keptNative", { location: profile.location }) : t("keptFolder");
  const deleted = t("deletedFolder", { location: profile.location });
  const note = el("p", "form-note", (purging ? deleted : kept) + rules);
  row.append(note);

  if (profile.canPurge) {
    const label = el("label", "form-check");
    const box = el("input");
    box.type = "checkbox";
    box.checked = purging;
    box.addEventListener("change", () => {
      purging = box.checked;
      render(overview);
    });
    label.append(box, el("span", "", t("alsoDelete")));
    row.append(label);
  }
  if (profile.teamRules > 0) {
    row.append(el("p", "form-error", count(profile.teamRules, "teamNeedsOne", "teamNeedsMany")));
  }

  const buttons = el("div", "form-buttons");
  const cancel = el("button", "quiet-button", t("cancel"));
  cancel.type = "button";
  cancel.addEventListener("click", () => {
    removing = null;
    render(overview);
  });
  const confirm = el(
    "button",
    "filled-button danger",
    purging ? t("removeAndDelete") : t("removeProfile"),
  );
  onClick(confirm, async () => {
    const purge = purging && profile.canPurge;
    removing = null;
    await api.removeProfile(profile.name, purge);
  });
  buttons.append(cancel, confirm);
  row.append(buttons);
  return row;
}

function profileRow(profile: Profile, overview: Overview): HTMLElement {
  if (removing === profile.name) return removeRow(profile, overview);
  const row = keyed(el("li", "profile"), `profile:${profile.name}`);
  row.style.setProperty("--hue", hue(profile.name));
  if (!profile.account) row.classList.add("signed-out");
  if (profile.isDefault) row.classList.add("is-default");

  // The whole left part is the main action: start Claude Code there.
  const open = el("button", "profile-open");
  open.append(el("span", "seal", profile.name.slice(0, 1).toUpperCase()));
  const who = el("span", "who");
  const title = el("span", "name-line");
  title.append(el("span", "name", profile.name));
  if (profile.plan) {
    // The account's subscription: a product name, the same in every language.
    const plan = el("span", "plan", profile.plan);
    plan.title = t("planHint", { plan: profile.plan });
    title.append(plan);
  }
  who.append(title, el("span", "account", profile.account ?? t("notSignedIn")));
  // The "…" says a question comes first: which project folder.
  open.append(who, el("span", "go", profile.account ? t("open") : t("signIn")));
  open.title = profile.account ? t("openHint", { account: profile.account }) : t("signInHint");
  onClick(open, async () => {
    await api.openClaude(profile.name, t("pickOpenTitle", { name: profile.name }));
  });
  row.append(open);

  if (profile.isDefault) {
    // Named after the profile: when the default moves, the tag and the
    // ring around the seal arrive on the new one.
    const tag = keyed(el("span", "default-tag", t("default")), `default:${profile.name}`);
    tag.title = t("defaultHint");
    row.append(tag);
  }

  if (!profile.isDefault) {
    // Words and not an icon: an icon alone did not say what it would do.
    const set = el("button", "set-default", t("setDefault"));
    set.title = t("setDefaultHint", { name: profile.name });
    onClick(set, () => api.setDefault(profile.name));
    row.append(set);
  }

  // Nothing is removed on this click: the row first asks to confirm.
  const remove = el("button", "remove");
  remove.type = "button";
  remove.title = t("removeProfileHint", { name: profile.name });
  remove.append(icon("bin", t("removeProfileLabel", { name: profile.name })));
  remove.addEventListener("click", () => {
    removing = profile.name;
    purging = false;
    closeForm();
    render(overview);
  });
  row.append(remove);
  return row;
}

/** The last line of the profile list: a way to add one. */
function newProfileRow(overview: Overview): HTMLElement {
  // A color no existing profile wears, so the form stands apart.
  const taken = new Set(assignHues(overview.profiles));
  const nextHue = HUES[freeHue("", taken)];

  if (!adding) {
    const row = keyed(el("li", "profile new signed-out"), "profile:+");
    row.style.setProperty("--hue", nextHue);
    const open = el("button", "profile-open");
    const who = el("span", "who");
    who.append(el("span", "name", t("newProfile")));
    open.append(el("span", "seal", "+"), who);
    open.type = "button";
    open.addEventListener("click", () => {
      adding = true;
      removing = null;
      // The first profile is usually the login already on this Mac.
      draftReuse = overview.canReuseLogin && overview.profiles.length === 0;
      render(overview);
    });
    row.append(open);
    return row;
  }

  const row = keyed(el("li", "profile-form signed-out"), "form:profile");
  row.style.setProperty("--hue", nextHue);
  const form = el("form");

  const line = el("div", "form-line");
  const seal = el("span", "seal", draftName.trim().slice(0, 1).toUpperCase() || "+");
  const input = el("input", "name-input");
  input.type = "text";
  input.value = draftName;
  input.placeholder = t("namePlaceholder");
  input.setAttribute("aria-label", t("nameLabel"));
  input.autocomplete = "off";
  input.spellcheck = false;
  input.maxLength = 40;
  input.addEventListener("input", () => {
    draftName = input.value;
    seal.textContent = draftName.trim().slice(0, 1).toUpperCase() || "+";
    // The message was about the previous name.
    formError = "";
    form.querySelector(".form-error")?.remove();
    void api.fit(Math.ceil(root.getBoundingClientRect().height));
  });
  line.append(seal, input);
  form.append(line);

  if (overview.canReuseLogin) {
    const label = el("label", "form-check");
    const box = el("input");
    box.type = "checkbox";
    box.checked = draftReuse;
    box.addEventListener("change", () => {
      draftReuse = box.checked;
    });
    label.append(box, el("span", "", t("reuseLogin")));
    form.append(label);
  }

  if (formError) {
    const error = el("p", "form-error", formError);
    error.setAttribute("role", "alert");
    form.append(error);
  }

  const buttons = el("div", "form-buttons");
  const cancel = el("button", "quiet-button", t("cancel"));
  cancel.type = "button";
  cancel.addEventListener("click", () => {
    closeForm();
    render(overview);
  });
  const create = el("button", "filled-button", t("createProfile"));
  create.type = "submit";
  buttons.append(cancel, create);
  form.append(buttons);

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    create.disabled = true;
    try {
      await api.addProfile(draftName, draftReuse);
      closeForm();
    } catch (error) {
      formError = String(error);
    }
    await refresh();
  });

  row.append(form);
  return row;
}

// The "new rule" form: open once a folder is chosen. Like the profile
// form, its state lives here because the page is rebuilt often.
let ruleDraft: RuleDraft | null = null;
let draftChoice = 0;
let draftProfile = "";
let draftLocked = false;
let ruleError = "";
// The place of the rule whose removal waits for a second click.
let removingRule: number | null = null;

function closeRuleForm(): void {
  ruleDraft = null;
  draftChoice = 0;
  draftLocked = false;
  ruleError = "";
}

/** Where a rule would apply, in words. The shape of the choice says which
 *  kind it is: a pattern ending in `/*` covers an owner's repositories. */
function choiceText(choice: { path: string | null; gitRemote: string | null }): string {
  if (choice.gitRemote?.endsWith("/*")) return t("choiceOwner", { owner: choice.gitRemote.slice(0, -2) });
  if (choice.gitRemote) return t("choiceRepo", { repo: choice.gitRemote });
  return t("choiceFolder", { path: choice.path ?? "" });
}

/** A conversation's title, or the start of its id when it has none. */
function titleOf(talk: { id: string; title: string | null }): string {
  return talk.title ?? t("untitled", { id: talk.id.slice(0, 8) });
}

/** The last line of the rules: a way to add one, then the form. */
function newRuleRow(overview: Overview): HTMLElement {
  if (!ruleDraft) {
    const row = keyed(el("li", "rule rule-new"), "rule:+");
    const open = el("button", "rule-add");
    open.title = t("newRuleHint");
    open.append(icon("plus"), el("span", "", t("newRule")));
    onClick(open, async () => {
      closeForm();
      removing = null;
      removingRule = null;
      const draft = await api.pickRuleFolder(t("pickRuleTitle"));
      if (!draft) return;
      closeRuleForm();
      ruleDraft = draft;
      // The profile the folder does not use yet is the likely answer:
      // start on the first one that is not the default.
      const other = overview.profiles.find((profile) => !profile.isDefault);
      draftProfile = (other ?? overview.profiles[0]).name;
    });
    row.append(open);
    return row;
  }

  const draft = ruleDraft;
  // A profile removed in a terminal meanwhile cannot stay selected.
  if (!overview.profiles.some((profile) => profile.name === draftProfile)) {
    draftProfile = overview.profiles[0].name;
  }
  const row = keyed(el("li", "profile-form rule-form"), "form:rule");
  row.style.setProperty("--hue", hue(draftProfile));
  const form = el("form");

  const where = el("fieldset", "rule-choices");
  where.append(el("legend", "", t("ruleWhere")));
  draft.choices.forEach((choice, index) => {
    const label = el("label", "form-check");
    const radio = el("input");
    radio.type = "radio";
    radio.name = "rule-where";
    radio.checked = index === draftChoice;
    radio.addEventListener("change", () => {
      draftChoice = index;
      ruleError = "";
      render(overview);
    });
    label.title = choice.gitRemote ? `git_remote = "${choice.gitRemote}"` : `path = "${choice.path}"`;
    label.append(radio, el("span", "", choiceText(choice)));
    where.append(label);
  });
  form.append(where);

  const use = el("label", "rule-use");
  const select = el("select", "rule-select");
  for (const profile of overview.profiles) {
    const option = el("option", "", profile.name);
    option.value = profile.name;
    option.selected = profile.name === draftProfile;
    select.append(option);
  }
  select.addEventListener("change", () => {
    draftProfile = select.value;
    ruleError = "";
    render(overview);
  });
  use.append(el("span", "", t("ruleProfile")), select);
  form.append(use);

  const lock = el("label", "form-check");
  lock.title = t("lockHint");
  const box = el("input");
  box.type = "checkbox";
  box.checked = draftLocked;
  box.addEventListener("change", () => {
    draftLocked = box.checked;
  });
  lock.append(box, el("span", "", t("lockIt")));
  form.append(lock);

  if (ruleError) {
    const error = el("p", "form-error", ruleError);
    error.setAttribute("role", "alert");
    form.append(error);
  }

  const buttons = el("div", "form-buttons");
  const cancel = el("button", "quiet-button", t("cancel"));
  cancel.type = "button";
  cancel.addEventListener("click", () => {
    closeRuleForm();
    render(overview);
  });
  const create = el("button", "filled-button", t("addRule"));
  create.type = "submit";
  buttons.append(cancel, create);
  form.append(buttons);

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    create.disabled = true;
    try {
      await api.addRule(draft.choices[draftChoice], draftProfile, draftLocked);
      closeRuleForm();
    } catch (error) {
      ruleError = String(error);
    }
    await refresh();
  });

  row.append(form);
  return row;
}

function ruleRow(rule: Rule, overview: Overview): HTMLElement {
  const row = keyed(
    el("li", "rule"),
    `rule:${rule.source}:${rule.gitRemote ?? ""}:${rule.path ?? ""}:${rule.profile}`,
  );
  row.style.setProperty("--hue", hue(rule.profile));
  const where = el("span", "rule-where");
  if (rule.gitRemote) {
    const part = el("span", "rule-part");
    part.append(icon("branch", t("gitRemote")), el("span", "rule-pattern", rule.gitRemote));
    where.append(part);
  }
  if (rule.path) {
    const part = el("span", "rule-part");
    part.append(icon("folder", t("folder")), el("span", "rule-pattern", rule.path));
    where.append(part);
  }
  // A dotted line from the place to the profile it leads to, as between a
  // chapter and its page number.
  const leader = el("span", "leader");
  leader.setAttribute("aria-hidden", "true");
  row.append(where, leader);

  if (rule.source === "team.toml") {
    const team = el("span", "rule-team", t("team"));
    team.title = t("teamHint");
    row.append(team);
  }
  if (rule.locked) {
    const lock = el("span", "rule-lock");
    lock.title = t("lockedHint");
    lock.append(icon("lock", t("locked")));
    row.append(lock);
  }
  const target = el("span", "rule-profile");
  target.append(el("span", "dot"), el("span", "", rule.profile));

  // Only your own rules can go: a team rule has no place of its own.
  const place = rule.index;
  if (place !== null && removingRule === place) {
    // Second click: it really goes.
    const go = keyed(el("button", "talk-go", t("removeRule")), `go:rule:${place}`);
    onClick(go, async () => {
      removingRule = null;
      await api.removeRule(rule);
    });
    row.append(go);
    return row;
  }
  if (place !== null) {
    const bin = el("button", "remove rule-bin");
    bin.type = "button";
    bin.title = t("removeThisRule");
    bin.append(icon("bin", t("removeThisRule")));
    bin.addEventListener("click", () => {
      removingRule = place;
      render(overview);
    });
    row.append(bin);
  }
  row.append(target);
  return row;
}

/** The app's mark, as in its icon: a filled seal in front of a hollow one. */
function mark(): SVGElement {
  const space = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(space, "svg");
  svg.setAttribute("viewBox", "0 0 22 16");
  svg.setAttribute("class", "mark");
  svg.setAttribute("aria-hidden", "true");
  keyed(svg, "mark");
  // The ring is an arc that stops short of the disc, so they never touch.
  const ring = document.createElementNS(space, "path");
  ring.setAttribute("d", "M12.6 3.1a5.6 5.6 0 1 1 0 9.8");
  ring.setAttribute("class", "mark-ring");
  // Its length counts as 1, so the stylesheet can draw it from 0 to 1.
  ring.setAttribute("pathLength", "1");
  const disc = document.createElementNS(space, "circle");
  disc.setAttribute("cx", "7.5");
  disc.setAttribute("cy", "8");
  disc.setAttribute("r", "6");
  disc.setAttribute("class", "mark-disc");
  svg.append(ring, disc);
  return svg;
}

function header(): HTMLElement {
  const top = el("header");
  top.append(mark(), el("h1", "", t("appName")));
  return top;
}

/** A line of the menu at the bottom, with its keyboard shortcut. */
function menuItem(label: string, keys: string, action: () => Promise<void>): HTMLElement {
  const item = el("button", "menu-item");
  item.append(el("span", "", label), el("kbd", "", keys));
  onClick(item, action);
  return item;
}

// The last measure of each profile's folder. It arrives after the list,
// and the previous one stays on screen until the new one is in.
let storage: StorageOverview | null = null;
// Folded until asked for: the sizes are a detail, not the reason the
// window was opened. Kept as long as the app runs.
let storageOpen = false;

let historyOpen = false;
// The profile whose conversations are listed under its storage line, and
// that list once it is read.
let pickedProfile: string | null = null;
let conversations: Conversations | null = null;
// The conversation whose move to the Trash waits for a second click.
let trashing: string | null = null;

/** Reads the conversations of the picked profile, then shows them. */
async function loadConversations(overview: Overview): Promise<void> {
  const name = pickedProfile;
  if (!name) return;
  try {
    const read = await api.conversations(name);
    // The user may have picked another profile meanwhile.
    if (pickedProfile !== name) return;
    conversations = read;
  } catch (error) {
    errorText = String(error);
    errorAt = Date.now();
  }
  render(overview);
}

/** The biggest conversations of a profile, each with a way to the Trash. */
function conversationList(name: string, overview: Overview): HTMLElement {
  const list = el("ul", "talks");
  if (!conversations) {
    list.append(el("li", "talks-note", t("reading")));
    return list;
  }
  if (conversations.items.length === 0) {
    list.append(el("li", "talks-note", t("noConversation")));
    return list;
  }
  for (const talk of conversations.items) {
    const row = keyed(el("li", "talk"), `talk:${name}:${talk.id}`);
    const text = el("span", "talk-text");
    const title = titleOf(talk);
    // Where it worked, when it has a folder, then when it was last used.
    const detail = [talk.folder, ago(talk.idleSecs)].filter(Boolean).join(", ");
    text.append(el("span", "talk-title", title), el("span", "talk-detail", detail));
    row.title = title;
    row.append(text);

    if (talk.inUse) {
      const busy = el("span", "talk-size", t("inUse"));
      busy.title = t("inUseHint");
      row.append(busy);
    } else if (trashing === talk.id) {
      // Second click: it really goes.
      const go = keyed(el("button", "talk-go", t("moveToTrash")), `go:talk:${talk.id}`);
      onClick(go, async () => {
        trashing = null;
        await api.trashConversation(name, talk.id);
        conversations = await api.conversations(name);
      });
      row.append(go);
    } else {
      row.append(el("span", "talk-size", bytes(talk.size)));
      const bin = el("button", "remove talk-bin");
      bin.type = "button";
      bin.title = t("trashHint");
      bin.append(icon("bin", t("trashLabel", { title })));
      bin.addEventListener("click", () => {
        trashing = talk.id;
        render(overview);
      });
      row.append(bin);
    }
    list.append(row);
  }
  if (conversations.more > 0) {
    list.append(el("li", "talks-note", count(conversations.more, "smallerOne", "smallerMany", { name })));
  }
  return list;
}

/** The line that folds and unfolds a section. Folded, it shows `sum`. */
function foldLine(
  id: string,
  label: string,
  hint: string,
  open: boolean,
  sum: string,
  flip: () => void,
  // True when the sum asks for a look: it is then written in amber.
  flagged = false,
): HTMLButtonElement {
  const toggle = keyed(el("button", "fold"), `fold:${id}`);
  toggle.type = "button";
  toggle.dataset.fold = id;
  toggle.title = hint;
  toggle.setAttribute("aria-expanded", String(open));
  toggle.append(icon("chevron"), el("span", "fold-title", label));
  if (!open) toggle.append(el("span", flagged ? "fold-sum fold-flag" : "fold-sum", sum));
  toggle.addEventListener("click", () => {
    flip();
    // The page was rebuilt: keep the keyboard on the line just used.
    document.querySelector<HTMLButtonElement>(`[data-fold="${id}"]`)?.focus();
  });
  return toggle;
}

// The choices offered for how long conversations are kept. Claude Code
// deletes them after 30 days unless told otherwise. Any other number of
// days goes in the field next to them.
const KEEP = [30, 90, 365];

/** The exact number of days, to read and to type. Enter, or leaving the
 *  field, applies it. */
function daysField(profile: Profile): HTMLElement {
  const field = el("label", "days-field");
  field.title = t("daysFieldHint");
  const input = el("input", "days-input");
  input.type = "number";
  input.min = "1";
  input.step = "1";
  input.inputMode = "numeric";
  input.value = String(profile.keepsDays);
  input.setAttribute("aria-label", t("daysFieldLabel", { name: profile.name }));
  input.addEventListener("change", async () => {
    const days = Number(input.value);
    if (days === profile.keepsDays) return;
    errorText = "";
    // What is not a whole number of days, 1 or more, is not sent: the
    // field goes back to the value in use.
    if (Number.isInteger(days) && days >= 1) {
      try {
        await api.setRetention(profile.name, days);
      } catch (error) {
        errorText = String(error);
        errorAt = Date.now();
      }
    }
    await refresh();
  });
  // Enter applies, like leaving the field.
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") input.blur();
  });
  field.append(input, el("span", "", t("daysUnit")));
  return field;
}

/** How long each profile keeps its conversations, with a way to change it. */
function historySection(overview: Overview): HTMLElement {
  const section = el("section", "storage-section");
  const kept = new Set(overview.profiles.map((profile) => profile.keepsDays));
  const only = overview.profiles[0].keepsDays;
  const sum = kept.size === 1 && only !== null ? days(only) : t("varies");
  section.append(
    foldLine(
      "history",
      t("kept"),
      t("keptHint"),
      historyOpen,
      sum,
      () => {
        historyOpen = !historyOpen;
        render(overview);
      },
    ),
  );
  if (!historyOpen) return section;

  const list = el("ul", "storage");
  for (const profile of overview.profiles) {
    const row = keyed(el("li", "storage-row"), `keep:${profile.name}`);
    row.style.setProperty("--hue", hue(profile.name));
    row.append(el("span", "storage-name", profile.name));
    if (profile.keepsDays === null) {
      // Its settings file cannot be read: say so, and offer nothing.
      const unknown = el("span", "keep-unknown", t("settingsUnreadable"));
      unknown.title = profile.keepsError ?? "";
      row.append(unknown);
    } else {
      const choices = el("span", "segments");
      choices.setAttribute("role", "group");
      choices.setAttribute("aria-label", t("keepGroup", { name: profile.name }));
      for (const choice of KEEP) {
        const button = el("button", "segment", days(choice));
        const current = profile.keepsDays === choice;
        button.setAttribute("aria-pressed", String(current));
        if (current) {
          button.type = "button";
          keyed(button, `kept:${profile.name}:${choice}`);
        } else {
          onClick(button, () => api.setRetention(profile.name, choice));
        }
        choices.append(button);
      }
      row.append(choices, daysField(profile));
    }
    list.append(row);
  }
  section.append(list);
  return section;
}

/** What takes a profile's space, biggest first: "3.4 GB conversations,
 *  41 MB plugins and skills". Empty when nothing is stored. */
function storageDetail(size: StorageOverview["profiles"][number]): string {
  const parts: [number, string][] = [
    [size.conversations, t("partConversations")],
    [size.fileHistory, t("partFileHistory")],
    [size.extensions, t("partExtensions")],
    [size.other, t("partOther")],
  ];
  return parts
    .filter(([amount]) => amount > 0)
    .sort((a, b) => b[0] - a[0])
    .map(([amount, what]) => `${bytes(amount)} ${what}`)
    .join(", ");
}

/** What a profile's folder holds, in the order the bar draws it. */
const PARTS = [
  ["conversations", "legendConversations"],
  ["fileHistory", "legendFileHistory"],
  ["extensions", "legendExtensions"],
  ["other", "legendOther"],
] as const;

/** The whole bar is one profile's folder, cut by what fills it. It says
 *  nothing about a limit, and does not compare profiles: the size beside
 *  it does. */
function partsBar(size: StorageOverview["profiles"][number]): HTMLElement {
  const bar = el("span", "bar bar-parts");
  if (size.total <= 0) return bar;
  // A part with anything at all keeps a visible sliver; the widths are
  // then brought back to a total of 100.
  const raw = PARTS.map(([part]) => (size[part] > 0 ? Math.max(size[part] / size.total, 0.02) : 0));
  const sum = raw.reduce((a, b) => a + b, 0);
  PARTS.forEach(([part, label], index) => {
    if (raw[index] === 0) return;
    const piece = el("span", `bar-fill bar-part part-${index}`);
    piece.style.width = `${((raw[index] / sum) * 100).toFixed(1)}%`;
    piece.dataset.bar = `size:${size.name}:${part}`;
    piece.title = `${t(label)}: ${bytes(size[part])}`;
    bar.append(piece);
  });
  return bar;
}

/** Which shade of the bar is what. */
function partsLegend(): HTMLElement {
  const legend = el("li", "legend");
  PARTS.forEach(([, label], index) => {
    const entry = el("span", "legend-entry");
    entry.append(el("span", `legend-dot part-${index}`), t(label));
    legend.append(entry);
  });
  return legend;
}

/** A line that unfolds to one line per profile: a bar cut by content, then
 *  the size. Folded, it shows the sum. */
function storageSection(measured: StorageOverview, overview: Overview): HTMLElement {
  const sizes = measured.profiles.filter((size) => hues.has(size.name));
  const section = el("section", "storage-section");
  section.append(
    foldLine(
      "storage",
      t("storage"),
      t("storageHint"),
      storageOpen,
      bytes(measured.total),
      () => {
        storageOpen = !storageOpen;
        render(overview);
      },
    ),
  );
  if (!storageOpen) return section;

  const list = el("ul", "storage");
  for (const size of sizes) {
    const item = keyed(el("li", "storage-item"), `size:${size.name}`);
    item.style.setProperty("--hue", hue(size.name));
    // The line is a button: it unfolds the profile's biggest conversations.
    const picked = pickedProfile === size.name;
    const row = el("button", "storage-row storage-pick");
    row.type = "button";
    row.setAttribute("aria-expanded", String(picked));
    row.title = `${storageDetail(size) || t("nothingStored")}\n${t("storageClick")}`;
    row.addEventListener("click", () => {
      pickedProfile = picked ? null : size.name;
      conversations = null;
      trashing = null;
      render(overview);
      void loadConversations(overview);
    });
    row.append(
      el("span", "storage-name", size.name),
      partsBar(size),
      el("span", "storage-size", size.total > 0 ? bytes(size.total) : t("empty")),
    );
    item.append(row);
    if (picked) item.append(conversationList(size.name, overview));
    list.append(item);
  }
  if (sizes.some((size) => size.total > 0)) list.append(partsLegend());
  section.append(list);
  return section;
}

// The last reading of what each profile consumed. Like the sizes, it
// arrives after the list and stays on screen until the next one is in.
let usage: UsageOverview | null = null;
let usageOpen = false;
// The profile whose projects and conversations are unfolded.
let usagePicked: string | null = null;

/** Why a folder gets its profile, as the end of a sentence. */
function reasonText(reason: Reason | null): string {
  switch (reason?.kind) {
    case "locked":
      return t("reasonLocked", { rule: reason.rule ?? "", source: reason.source ?? "" });
    case "rule":
      return t("reasonRule", { rule: reason.rule ?? "", source: reason.source ?? "" });
    case "file":
      return t("reasonFile", { file: reason.file ?? "" });
    case "explicit":
      return t("reasonExplicit");
    default:
      return t("reasonDefault");
  }
}

/** What today's rules say about a project, in a few words. Empty when a
 *  rule covers it. */
function projectNote(project: UsageProject): string {
  switch (project.state) {
    case "covered":
      return "";
    case "default":
      return t("noteDefault");
    case "elsewhere":
      return t("noteElsewhere", { profile: project.profile ?? "" });
    case "gone":
      return t("noteGone");
    default:
      return t("noteUnknown");
  }
}

/** The same in full, shown on hover. `name` is the profile that was used. */
function projectWhy(project: UsageProject, name: string): string {
  switch (project.state) {
    case "covered":
      return t("whyCovered", { name, reason: reasonText(project.reason) });
    case "default":
      return t("whyDefault", { name });
    case "elsewhere":
      return t("whyElsewhere", { name, profile: project.profile ?? "", reason: reasonText(project.reason) });
    case "gone":
      return t("whyGone");
    default:
      return project.error ?? "";
  }
}

/** What one profile was used for: its projects against the rules, then
 *  the conversations that send the most with each answer. */
function usageDetail(used: Usage): HTMLElement {
  const list = el("ul", "talks");
  if (used.projects.length === 0) {
    list.append(el("li", "talks-note", t("noActivity")));
    return list;
  }
  list.append(el("li", "talks-note", t("projectsHeading")));
  for (const project of used.projects) {
    const row = keyed(el("li", "talk used"), `project:${used.name}:${project.folder}`);
    row.title = `${project.folder}\n${projectWhy(project, used.name)}`;
    // The line is tinted up to the project's part of this profile's usage.
    row.style.setProperty("--part", `${(project.share * 100).toFixed(1)}%`);
    row.append(el("span", "used-share", percent(project.share)), el("span", "used-folder", project.folder));
    const note = projectNote(project);
    if (note) {
      row.append(el("span", project.flagged ? "used-note used-flag" : "used-note", note));
    } else {
      // A rule covers it: nothing to read, a quiet mark is enough.
      const ok = el("span", "used-note used-ok", "✓");
      ok.setAttribute("aria-label", t("covered"));
      row.append(ok);
    }
    list.append(row);
  }
  if (used.moreProjects > 0) {
    list.append(
      el("li", "talks-note", count(used.moreProjects, "lighterOne", "lighterMany", { name: used.name })),
    );
  }

  const heading = el("li", "talks-note", t("talksHeading"));
  if (used.reread !== null) heading.title = t("rereadHint", { share: percent(used.reread) });
  list.append(heading);
  for (const talk of used.talks) {
    const row = keyed(el("li", "talk"), `context:${used.name}:${talk.id}`);
    const text = el("span", "talk-text");
    const title = titleOf(talk);
    const detail = t("talkDetail", { folder: talk.folder, share: percent(talk.share) });
    text.append(el("span", "talk-title", title), el("span", "talk-detail", detail));
    const sent = tokens(talk.context);
    const context = el("span", talk.heavy ? "talk-size used-flag" : "talk-size", sent);
    row.title = `${title}\n${t(talk.heavy ? "talkHeavy" : "talkSends", { context: sent })}`;
    row.append(text, context);
    list.append(row);
  }
  return list;
}

// The limits Claude Code last reported for each profile, by profile name.
let limits = new Map<string, Limits>();

/** One limit, as Claude shows it: its name, how much is used and when it
 *  starts over, then a bar where full is the limit itself. */
function limitLine(name: string, label: string, kind: string, window: LimitWindow): HTMLElement {
  const line = el("div", "limit");
  if (window.used >= 90) line.classList.add("limit-high");
  else if (window.used >= 70) line.classList.add("limit-near");
  const used = percent(window.used / 100);
  // Claude Desktop gives the number without its reset time: then the
  // number stands alone.
  let fact = used;
  if (window.resetsIn !== null) fact = t("limitFact", { used, when: until(window.resetsIn) });
  else if (window.startedOver) fact = t("limitFresh", { used });
  const bar = el("span", "bar");
  const fill = el("span", "bar-fill");
  fill.style.width = `${(window.used > 0 ? Math.max(window.used, 2) : 0).toFixed(1)}%`;
  fill.dataset.bar = `limit:${name}:${kind}`;
  bar.append(fill);
  line.append(el("span", "limit-name", label), el("span", "limit-fact", fact), bar);
  return line;
}

/** Under a profile in "Usage": its account's 5-hour and weekly limits, or
 *  the way to get them. Nothing when its settings cannot be read. */
function limitsBlock(name: string): HTMLElement | null {
  const found = limits.get(name);
  if (!found || found.state === "unknown") return null;
  const known = found.fiveHour !== null || found.sevenDay !== null;
  const block = keyed(el("div", "limits"), `limits:${name}:${found.state}:${known}`);
  // Turns Claude Code's reports on or off for this profile.
  const toggle = (label: string, hint: string, on: boolean): HTMLButtonElement => {
    const button = el("button", "limits-action", label);
    button.title = hint;
    onClick(button, async () => {
      await api.setLimits(name, on);
    });
    return button;
  };
  if (!known && found.state === "off") {
    block.append(toggle(t("limitsShow"), t("limitsShowHint"), true));
    return block;
  }
  block.title = t("limitsHint");
  if (found.fiveHour) block.append(limitLine(name, t("limit5h"), "5h", found.fiveHour));
  if (found.sevenDay) block.append(limitLine(name, t("limitWeek"), "week", found.sevenDay));
  const foot = el("div", "limits-foot");
  // Product names, the same in every language.
  const source = found.source === "desktop" ? "Claude Desktop" : "Claude Code";
  foot.append(
    el(
      "span",
      "limits-note",
      found.ageSecs === null
        ? t("limitsWaiting")
        : t("limitsUpdated", { when: ago(found.ageSecs), source }),
    ),
  );
  // Claude Desktop alone already gives numbers, for the account it is
  // logged in to; Claude Code adds the reset times, and the other accounts.
  foot.append(
    found.state === "on"
      ? toggle(t("limitsHide"), t("limitsHideHint"), false)
      : toggle(t("limitsShowToo"), t("limitsShowHint"), true),
  );
  block.append(foot);
  return block;
}

/** A line that unfolds to one line per profile: its part of the usage of
 *  the last days. Folded, it says how many projects deserve a look. */
function usageSection(read: UsageOverview, overview: Overview): HTMLElement {
  const profiles = read.profiles.filter((used) => hues.has(used.name));
  const flagged = profiles.reduce((sum, used) => sum + used.flagged, 0);
  const section = el("section", "storage-section");
  const period = t("lastDays", { n: read.days });
  section.append(
    foldLine(
      "usage",
      t("usage"),
      t("usageHint", { n: read.days }),
      usageOpen,
      flagged > 0 ? t("toCheck", { n: flagged }) : period,
      () => {
        usageOpen = !usageOpen;
        render(overview);
      },
      flagged > 0,
    ),
  );
  if (!usageOpen) return section;

  const list = el("ul", "storage");
  // A part of the whole only means something when there is more than one
  // part: with a single profile in use, the bar would always be full.
  const compared = profiles.filter((used) => used.weight > 0).length > 1;
  for (const used of profiles) {
    const item = keyed(el("li", "storage-item"), `used:${used.name}`);
    item.style.setProperty("--hue", hue(used.name));
    const picked = usagePicked === used.name;
    const row = el("button", "storage-row storage-pick");
    row.type = "button";
    row.setAttribute("aria-expanded", String(picked));
    row.title =
      used.flagged > 0
        ? `${count(used.flagged, "projectsToCheckOne", "projectsToCheckMany")}\n${t("usageClick")}`
        : t("usageClick");
    row.addEventListener("click", () => {
      usagePicked = picked ? null : used.name;
      render(overview);
    });
    row.append(el("span", "storage-name", used.name));
    // No bar here: the bars of this section are the account's limits,
    // below. The profile's part of all usage is a number, and only when
    // there is more than one profile to compare.
    const projects = used.projects.length + used.moreProjects;
    let fact = used.share === null ? t("idle") : count(projects, "projectsOne", "projectsMany");
    if (compared && used.share !== null) {
      fact = t("shareFact", { share: percent(used.share), projects: fact });
      row.title = `${t("shareHint", { n: read.days })}\n${row.title}`;
    }
    row.append(el("span", "storage-fact", fact));
    item.append(row);
    const block = limitsBlock(used.name);
    if (block) item.append(block);
    if (picked) item.append(usageDetail(used));
    list.append(item);
  }
  section.append(list);
  return section;
}

// The terminal apps that can be chosen, read when the section unfolds.
let terminalOpen = false;
let terminalList: Terminal[] | null = null;

/** Which terminal app "Open…" starts Claude Code in. Folded, it names the
 *  one in use. */
function terminalSection(overview: Overview): HTMLElement {
  const section = el("section", "storage-section");
  section.append(
    foldLine(
      "terminal",
      t("terminal"),
      t("terminalHint"),
      terminalOpen,
      overview.terminal ?? t("systemDefault"),
      () => {
        terminalOpen = !terminalOpen;
        terminalList = null;
        render(overview);
        if (!terminalOpen) return;
        void api.terminals().then((found) => {
          terminalList = found;
          render(overview);
        });
      },
    ),
  );
  if (!terminalOpen) return section;

  const list = el("ul", "storage");
  if (!terminalList) {
    list.append(el("li", "talks-note", t("lookingTerminals")));
    section.append(list);
    return section;
  }
  const choice = (label: string, hint: string, selected: boolean, path: string | null) => {
    const item = keyed(el("li", "storage-item"), `terminal:${path ?? ""}`);
    const row = el("button", "storage-row storage-pick pick-row");
    row.setAttribute("role", "menuitemradio");
    row.setAttribute("aria-checked", String(selected));
    row.title = hint;
    const check = el("span", "pick-mark", selected ? "✓" : "");
    if (selected) keyed(check, `picked:${path ?? ""}`);
    row.append(check, el("span", "pick-name", label));
    if (selected) {
      row.type = "button";
    } else {
      onClick(row, async () => {
        await api.setTerminal(path);
        terminalList = await api.terminals();
      });
    }
    item.append(row);
    return item;
  };
  list.append(
    choice(
      t("systemDefault"),
      t("systemDefaultHint"),
      !terminalList.some((terminal) => terminal.selected),
      null,
    ),
    ...terminalList.map((terminal) => choice(terminal.name, terminal.path, terminal.selected, terminal.path)),
    el("li", "talks-note", t("terminalsNote")),
  );
  section.append(list);
  return section;
}

// The editors of the VS Code family found on this Mac, read after the
// list of profiles; null until then.
let editorsOpen = false;
let editorList: Editor[] | null = null;

/** Whether the Claude Code extension of each editor goes through Orberus,
 *  with the switch. Folded, it says how many editors still skip it. */
function editorsSection(found: Editor[], overview: Overview): HTMLElement {
  const skipping = found.filter((editor) => editor.state === "off").length;
  const section = el("section", "storage-section");
  section.append(
    foldLine(
      "editors",
      t("editors"),
      t("editorsHint"),
      editorsOpen,
      skipping > 0
        ? count(skipping, "editorsSkipOne", "editorsSkipMany")
        : found.some((editor) => editor.state === "on")
          ? t("editorsAllOn")
          : t("editorsNone"),
      () => {
        editorsOpen = !editorsOpen;
        render(overview);
      },
      skipping > 0,
    ),
  );
  if (!editorsOpen) return section;

  const list = el("ul", "storage");
  for (const editor of found) {
    const item = keyed(el("li", "storage-item"), `editor:${editor.settings}`);
    const row = el("div", "storage-row editor-row");
    const file = editor.settingsShort;
    const fact = (text: string, hint: string, flagged = false) => {
      row.title = hint;
      // Keyed by its state: the words pop in when the switch is used.
      return keyed(
        el("span", flagged ? "editor-state editor-skips" : "editor-state", text),
        `editor-state:${editor.settings}:${editor.state}`,
      );
    };
    const action = (label: string, hint: string, on: boolean) => {
      const button = el("button", "limits-action editor-action", label);
      button.title = hint;
      onClick(button, async () => {
        await api.setEditor(editor.settings, on);
      });
      return button;
    };
    row.append(el("span", "pick-name", editor.name));
    switch (editor.state) {
      case "on":
        row.append(
          fact(t("editorOn"), t("editorOnHint", { file })),
          action(t("editorTurnOff"), t("editorTurnOffHint"), false),
        );
        break;
      case "off":
        row.append(
          fact(t("editorOff"), t("editorOffHint", { file }), true),
          action(t("editorTurnOn"), t("editorTurnOnHint"), true),
        );
        break;
      case "absent":
        row.append(fact(t("editorAbsent"), t("editorAbsentHint", { file })));
        break;
      case "other":
        row.append(
          fact(
            // The program's name is enough on the line; its path is in the hint.
            t("editorOther", { program: (editor.detail ?? "").replace(/^.*\//, "") }),
            `${t("editorOtherHint", { file })}\n${editor.detail ?? ""}`,
          ),
        );
        break;
      case "unreadable":
        row.append(
          fact(t("editorUnreadable"), t("editorUnreadableHint", { why: editor.detail ?? "", file }), true),
        );
        break;
    }
    item.append(row);
    list.append(item);
  }
  list.append(el("li", "talks-note", t("editorsNote")));
  section.append(list);
  return section;
}

// True right after a switch was used: the next page slides its knob from
// the other side. Not at the opening of the window, where nothing moved.
let switched = false;

/** A setting that is on or off, as a switch at the end of its line. The
 *  whole line is the button, so it is as easy to hit as its neighbours. */
function menuSwitch(label: string, hint: string, on: boolean, action: () => Promise<void>): HTMLElement {
  const item = el("button", "menu-item menu-switch");
  item.setAttribute("role", "switch");
  item.setAttribute("aria-checked", String(on));
  item.title = hint;
  const track = el("span", switched ? "switch moved" : "switch");
  switched = false;
  track.append(el("span", "switch-knob"));
  item.append(el("span", "", label), track);
  onClick(item, async () => {
    await action();
    switched = true;
  });
  return item;
}

// The version to update to, once GitHub has answered; null when this one
// is the latest, or when nothing is known.
let newVersion: string | null = null;

/** The line that says a later release exists. It opens the page that
 *  explains the update: the app does not replace itself. */
function updateItem(version: string): HTMLElement {
  const item = keyed(el("button", "menu-item menu-update"), "update");
  item.title = t("updateHint", { version });
  item.append(el("span", "", t("updateAvailable", { version })), el("span", "update-badge", t("updateNew")));
  onClick(item, () => api.openUpdatePage());
  return item;
}

/** The last line of the window: what the app is not, and which version it
 *  is. The version is the first thing asked for in a bug report. */
function legalLine(overview: Overview | null): HTMLElement {
  const line = el("p", "legal");
  line.append(el("span", "", t("legal")));
  if (overview) {
    const version = el("span", "version", t("version", { version: overview.version }));
    version.title = t("versionHint");
    line.append(version);
  }
  return line;
}

function menu(overview: Overview | null): HTMLElement {
  const footer = el("footer");
  if (newVersion) footer.append(updateItem(newVersion));
  if (overview) {
    const on = overview.openAtLogin;
    const paused = overview.paused;
    footer.append(menuSwitch(t("pause"), t("pauseHint"), paused, () => api.setPaused(!paused)));
    footer.append(menuSwitch(t("openAtLogin"), t("openAtLoginHint"), on, () => api.setOpenAtLogin(!on)));
  }
  footer.append(
    menuItem(t("editConfig"), "⌘,", () => api.openConfig()),
    menuItem(t("quit"), "⌘Q", () => api.quit()),
    legalLine(overview),
  );
  return footer;
}

/** Said at the top of the window for as long as the pause lasts, with the
 *  way out next to it: a pause is easy to forget. */
function pausedNotice(): HTMLElement {
  const node = notice(t("pausedNotice"), "warn");
  const resume = el("button", "notice-action", t("resume"));
  onClick(resume, async () => {
    await api.setPaused(false);
    switched = true;
  });
  node.append(resume);
  return node;
}

function notice(text: string, kind = ""): HTMLElement {
  const node = keyed(el("p", `notice ${kind}`.trim(), text), `notice:${text}`);
  if (kind === "error") node.setAttribute("role", "alert");
  return node;
}

// What the last page showed: the named elements, the width of each bar and
// which sections were unfolded. `settle` compares the new page with it.
let seen = new Set<string>();
let widths = new Map<string, string>();
let folds = new Map<string, boolean>();

/** Gives the new page its motion. Only what changed moves: an element not
 *  on the last page plays its entrance, a bar slides from its last width,
 *  an arrow turns when its section was just folded or unfolded. The layout
 *  is final at once (the window is sized on it); the motion is only
 *  opacity and small shifts on top. */
function settle(page: HTMLElement): void {
  const now = new Set<string>();
  // The place among what appears together, so entrances follow each other
  // from top to bottom.
  let order = 0;
  for (const node of page.querySelectorAll<HTMLElement | SVGElement>("[data-key]")) {
    const key = node.getAttribute("data-key") ?? "";
    now.add(key);
    if (seen.has(key)) continue;
    node.classList.add("enter");
    node.style.setProperty("--i", String(order));
    order += 1;
  }
  seen = now;

  const drawn = new Map<string, string>();
  for (const fill of page.querySelectorAll<HTMLElement>("[data-bar]")) {
    const key = fill.dataset.bar ?? "";
    const target = fill.style.width;
    const from = widths.get(key) ?? "0%";
    drawn.set(key, target);
    if (from === target) continue;
    fill.style.width = from;
    // Reading a measure makes the browser take the starting width into
    // account; without it only the final width would ever be drawn.
    void fill.offsetWidth;
    fill.style.width = target;
  }
  widths = drawn;

  const open = new Map<string, boolean>();
  for (const fold of page.querySelectorAll<HTMLElement>("[data-fold]")) {
    const id = fold.dataset.fold ?? "";
    const state = fold.getAttribute("aria-expanded") === "true";
    open.set(id, state);
    if (folds.has(id) && folds.get(id) !== state) fold.classList.add("turned");
  }
  folds = open;
}

function render(overview: Overview | null, loadError = ""): void {
  const page = el("div", "page");
  page.append(header());

  if (loadError) page.append(notice(loadError, "error"));
  if (errorText) page.append(notice(errorText, "error"));

  if (overview) {
    assignHues(overview.profiles);
    if (!overview.shimInstalled) {
      page.append(notice(t("shimMissing")));
    }
    if (overview.paused) page.append(pausedNotice());
    const main = el("main");
    if (overview.profiles.length === 0 && !adding) {
      main.append(el("p", "empty", t("noProfile")));
    }
    const list = el("ul", "profiles");
    list.append(
      ...overview.profiles.map((profile) => profileRow(profile, overview)),
      newProfileRow(overview),
    );
    main.append(list);
    // A rule needs a profile to lead to: no section before the first one.
    if (overview.profiles.length > 0) {
      const section = el("section", "rules-section");
      const title = keyed(el("h2", "", t("rules")), "title:rules");
      title.title = t("rulesHint");
      const list = el("ul", "rules");
      list.append(...overview.rules.map((rule) => ruleRow(rule, overview)), newRuleRow(overview));
      section.append(title, list);
      main.append(section);
    }
    // Only the profiles still in the list: a measure can be a moment old.
    if (usage && usage.profiles.some((used) => hues.has(used.name))) {
      main.append(usageSection(usage, overview));
    }
    if (overview.profiles.length > 0) main.append(historySection(overview));
    if (storage && storage.profiles.some((size) => hues.has(size.name))) {
      main.append(storageSection(storage, overview));
    }
    // Only where an editor of the VS Code family is installed.
    if (overview.profiles.length > 0 && editorList && editorList.length > 0) {
      main.append(editorsSection(editorList, overview));
    }
    if (overview.profiles.length > 0) main.append(terminalSection(overview));
    page.append(main);
  }

  page.append(menu(overview));
  root.replaceChildren(page);
  root.classList.remove("away");
  settle(page);
  // Rebuilding the page drops the cursor: put it back in the form.
  page.querySelector<HTMLInputElement>(".name-input")?.focus();
  // The window takes the height of what it shows, like a native menu.
  void api.fit(Math.ceil(page.getBoundingClientRect().height));
}

async function refresh(): Promise<void> {
  let overview: Overview;
  try {
    overview = await api.overview();
  } catch (error) {
    // The config could not be read: say why instead of showing stale data.
    render(null, String(error));
    return;
  }
  render(overview);
  // The limits are a few small files: read before the slow measures.
  try {
    const read = await api.limits();
    const changed = JSON.stringify(read) !== JSON.stringify([...limits.values()]);
    limits = new Map(read.map((found) => [found.name, found]));
    if (changed && !adding && !removing && !ruleDraft) render(overview);
  } catch {
    // Keep the last reading.
  }
  // The editors: a few folders to look at, still quick.
  try {
    const found = await api.editors();
    const changed = JSON.stringify(found) !== JSON.stringify(editorList);
    editorList = found;
    if (changed && !adding && !removing && !ruleDraft) render(overview);
  } catch {
    // Keep the last list.
  }
  // Measuring reads whole folders: the list above does not wait for it.
  // Meanwhile, is there a newer release? Not awaited: the answer can take
  // seconds on a slow network, and the measures do not depend on it.
  void api
    .update()
    .then((version) => {
      if (version === newVersion) return;
      newVersion = version;
      if (!adding && !removing && !ruleDraft) render(overview);
    })
    .catch(() => {
      // No answer is not worth a message.
    });
  try {
    const measured = await api.storage();
    const changed = JSON.stringify(measured) !== JSON.stringify(storage);
    storage = measured;
    // Not while a form is open: rebuilding the page would disturb typing.
    if (changed && !adding && !removing && !ruleDraft) render(overview);
  } catch {
    // Sizes are a comfort, not worth an error message: keep the last ones.
  }
  // The same for the usage, read last because it is the slowest.
  try {
    const read = await api.usage();
    const changed = JSON.stringify(read) !== JSON.stringify(usage);
    usage = read;
    if (changed && !adding && !removing && !ruleDraft) render(overview);
  } catch {
    // Keep the last reading.
  }
}

// The config can change in a terminal while the popover is closed, so the
// list is read again each time the window comes back to the front.
window.addEventListener("focus", () => {
  // An error stays long enough to be read: a failed action can move the
  // focus away and back within the same second.
  if (Date.now() - errorAt > 10_000) errorText = "";
  void refresh();
});

// The window closes when it loses the focus. Forget what was on screen, so
// the next opening plays its entrances, and blank the page meanwhile:
// otherwise the old page would flash before them. Not while a folder
// picker or a form is open, where the window stays or comes straight back.
window.addEventListener("blur", () => {
  if (pending > 0 || adding || removing || ruleDraft) return;
  seen = new Set();
  widths = new Map();
  if (insideApp) root.classList.add("away");
});

// A safety net: should the window ever be shown without taking the focus,
// the first move of the pointer brings the page back.
let waking = false;
window.addEventListener("pointermove", () => {
  if (waking || !root.classList.contains("away")) return;
  waking = true;
  void refresh().finally(() => {
    waking = false;
  });
});

window.addEventListener("keydown", (event) => {
  // The shortcuts written next to the menu lines, and ⌘N for a new profile.
  if (event.metaKey && !event.altKey && !event.ctrlKey) {
    const key = event.key.toLowerCase();
    if (key === "q") {
      event.preventDefault();
      void api.quit();
    } else if (key === ",") {
      event.preventDefault();
      api.openConfig().catch((error) => {
        errorText = String(error);
        errorAt = Date.now();
        void refresh();
      });
    } else if (key === "n" && !adding) {
      event.preventDefault();
      adding = true;
      removing = null;
      void refresh();
    }
    return;
  }
  if (event.key !== "Escape") return;
  // Escape first backs out of a form or a question, then closes the window.
  if (adding || removing || trashing || ruleDraft || removingRule !== null) {
    closeForm();
    closeRuleForm();
    removing = null;
    removingRule = null;
    trashing = null;
    void refresh();
  } else {
    void api.hide();
  }
});

// The page says which language it speaks, for what the native side
// writes: the document itself, and the menu under a right click.
document.documentElement.lang = language;
void api.setQuitLabel(t("quit")).catch(() => {});

void refresh();
