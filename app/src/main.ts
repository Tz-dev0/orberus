// The popover under the menu bar icon: profiles, rules, and a few actions.
// Plain TypeScript and DOM calls: the page is one small list, a framework
// would add more code than it removes.

import {
  api,
  insideApp,
  type Overview,
  type Profile,
  type Conversations,
  type Rule,
  type RuleDraft,
  type StorageOverview,
  type Usage,
  type UsageOverview,
} from "./api";

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
    errorText = "";
    try {
      await action();
    } catch (error) {
      errorText = String(error);
      errorAt = Date.now();
    } finally {
      node.disabled = false;
    }
    await refresh();
  });
}

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

/** How many rules, in words: "1 rule", "2 rules". */
function count(rules: number): string {
  return rules === 1 ? "1 rule" : `${rules} rules`;
}

/** Takes the place of a profile's row while its removal is confirmed. */
function removeRow(profile: Profile, overview: Overview): HTMLElement {
  const row = el("li", "profile-form");
  row.style.setProperty("--hue", hue(profile.name));
  if (!profile.account) row.classList.add("signed-out");

  const line = el("div", "form-line");
  line.append(
    el("span", "seal", profile.name.slice(0, 1).toUpperCase()),
    el("span", "name", `Remove ${profile.name}?`),
  );
  row.append(line);

  // Say exactly what goes and what stays, before the click.
  const own = profile.rules - profile.teamRules;
  const rules = own > 0 ? ` Your ${count(own)} for it will be removed too.` : "";
  const kept = profile.canPurge
    ? "Its Claude Code folder stays on this Mac, with its login and history."
    : `Its folder ${profile.location} is your own Claude Code setup: it stays on this Mac.`;
  const deleted = `${profile.location} will be deleted with its history and settings, and its login removed from the keychain. This cannot be undone.`;
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
    label.append(box, el("span", "", "Also delete its folder and sign out"));
    row.append(label);
  }
  if (profile.teamRules > 0) {
    const needs = profile.teamRules === 1 ? "needs" : "need";
    row.append(
      el(
        "p",
        "form-error",
        `${count(profile.teamRules)} from your team ${needs} this profile: Claude Code will refuse to start where ${profile.teamRules === 1 ? "it applies" : "they apply"}.`,
      ),
    );
  }

  const buttons = el("div", "form-buttons");
  const cancel = el("button", "quiet-button", "Cancel");
  cancel.type = "button";
  cancel.addEventListener("click", () => {
    removing = null;
    render(overview);
  });
  const confirm = el(
    "button",
    "filled-button danger",
    purging ? "Remove and delete" : "Remove profile",
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
  const row = el("li", "profile");
  row.style.setProperty("--hue", hue(profile.name));
  if (!profile.account) row.classList.add("signed-out");
  if (profile.isDefault) row.classList.add("is-default");

  // The whole left part is the main action: start Claude Code there.
  const open = el("button", "profile-open");
  open.append(el("span", "seal", profile.name.slice(0, 1).toUpperCase()));
  const who = el("span", "who");
  who.append(
    el("span", "name", profile.name),
    el("span", "account", profile.account ?? "Not signed in"),
  );
  // The "…" says a question comes first: which project folder.
  open.append(who, el("span", "go", profile.account ? "Open…" : "Sign in…"));
  open.title = profile.account
    ? `Choose a project folder, then open Claude Code there as ${profile.account}`
    : "Choose a project folder, then open Claude Code there to sign in";
  onClick(open, async () => {
    await api.openClaude(profile.name);
  });
  row.append(open);

  if (profile.isDefault) {
    const tag = el("span", "default-tag", "Default");
    tag.title = "Used where no rule applies";
    row.append(tag);
  }

  if (!profile.isDefault) {
    // Words and not an icon: an icon alone did not say what it would do.
    const set = el("button", "set-default", "Set as default");
    set.title = `Use ${profile.name} where no rule applies`;
    onClick(set, () => api.setDefault(profile.name));
    row.append(set);
  }

  // Nothing is removed on this click: the row first asks to confirm.
  const remove = el("button", "remove");
  remove.type = "button";
  remove.title = `Remove ${profile.name}…`;
  remove.append(icon("bin", `Remove ${profile.name}`));
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
    const row = el("li", "profile new signed-out");
    row.style.setProperty("--hue", nextHue);
    const open = el("button", "profile-open");
    const who = el("span", "who");
    who.append(el("span", "name", "New profile…"));
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

  const row = el("li", "profile-form signed-out");
  row.style.setProperty("--hue", nextHue);
  const form = el("form");

  const line = el("div", "form-line");
  const seal = el("span", "seal", draftName.trim().slice(0, 1).toUpperCase() || "+");
  const input = el("input", "name-input");
  input.type = "text";
  input.value = draftName;
  input.placeholder = "Name, for example work";
  input.setAttribute("aria-label", "Profile name");
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
    label.append(box, el("span", "", "Use the Claude Code login already on this Mac"));
    form.append(label);
  }

  if (formError) {
    const error = el("p", "form-error", formError);
    error.setAttribute("role", "alert");
    form.append(error);
  }

  const buttons = el("div", "form-buttons");
  const cancel = el("button", "quiet-button", "Cancel");
  cancel.type = "button";
  cancel.addEventListener("click", () => {
    closeForm();
    render(overview);
  });
  const create = el("button", "filled-button", "Create profile");
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

/** The last line of the rules: a way to add one, then the form. */
function newRuleRow(overview: Overview): HTMLElement {
  if (!ruleDraft) {
    const row = el("li", "rule rule-new");
    const open = el("button", "rule-add");
    open.title = "Choose a folder, then where the rule applies and which profile it uses";
    open.append(icon("plus"), el("span", "", "New rule…"));
    onClick(open, async () => {
      closeForm();
      removing = null;
      removingRule = null;
      const draft = await api.pickRuleFolder();
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
  const row = el("li", "profile-form rule-form");
  row.style.setProperty("--hue", hue(draftProfile));
  const form = el("form");

  const where = el("fieldset", "rule-choices");
  where.append(el("legend", "", "Where the rule applies"));
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
    label.append(radio, el("span", "", choice.label));
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
  use.append(el("span", "", "Profile to use there"), select);
  form.append(use);

  const lock = el("label", "form-check");
  lock.title = "A locked rule wins over everything: Claude Code does not start there with another profile";
  const box = el("input");
  box.type = "checkbox";
  box.checked = draftLocked;
  box.addEventListener("change", () => {
    draftLocked = box.checked;
  });
  lock.append(box, el("span", "", "Lock it: refuse any other profile there"));
  form.append(lock);

  if (ruleError) {
    const error = el("p", "form-error", ruleError);
    error.setAttribute("role", "alert");
    form.append(error);
  }

  const buttons = el("div", "form-buttons");
  const cancel = el("button", "quiet-button", "Cancel");
  cancel.type = "button";
  cancel.addEventListener("click", () => {
    closeRuleForm();
    render(overview);
  });
  const create = el("button", "filled-button", "Add rule");
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
  const row = el("li", "rule");
  const where = el("span", "rule-where");
  if (rule.gitRemote) {
    const part = el("span", "rule-part");
    part.append(icon("branch", "Git remote"), el("span", "rule-pattern", rule.gitRemote));
    where.append(part);
  }
  if (rule.path) {
    const part = el("span", "rule-part");
    part.append(icon("folder", "Folder"), el("span", "rule-pattern", rule.path));
    where.append(part);
  }
  row.append(where);

  if (rule.source === "team.toml") {
    const team = el("span", "rule-team", "Team");
    team.title = "Provided by your company in team.toml";
    row.append(team);
  }
  if (rule.locked) {
    const lock = el("span", "rule-lock");
    lock.title = "Locked: no other profile can be used there";
    lock.append(icon("lock", "Locked"));
    row.append(lock);
  }
  const target = el("span", "rule-profile");
  target.style.setProperty("--hue", hue(rule.profile));
  target.append(el("span", "dot"), el("span", "", rule.profile));

  // Only your own rules can go: a team rule has no place of its own.
  const place = rule.index;
  if (place !== null && removingRule === place) {
    // Second click: it really goes.
    const go = el("button", "talk-go", "Remove rule");
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
    bin.title = "Remove this rule";
    bin.append(icon("bin", "Remove this rule"));
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
  // The ring is an arc that stops short of the disc, so they never touch.
  const ring = document.createElementNS(space, "path");
  ring.setAttribute("d", "M12.6 3.1a5.6 5.6 0 1 1 0 9.8");
  ring.setAttribute("class", "mark-ring");
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
  top.append(mark(), el("h1", "", "Claude Profiles"));
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
    list.append(el("li", "talks-note", "Reading…"));
    return list;
  }
  if (conversations.items.length === 0) {
    list.append(el("li", "talks-note", "No conversation on disk."));
    return list;
  }
  for (const talk of conversations.items) {
    const row = el("li", "talk");
    const text = el("span", "talk-text");
    text.append(el("span", "talk-title", talk.title), el("span", "talk-detail", talk.detail));
    row.title = talk.title;
    row.append(text);

    if (talk.inUse) {
      const busy = el("span", "talk-size", "In use?");
      busy.title = "Changed in the last hour: it may still be open, so it stays";
      row.append(busy);
    } else if (trashing === talk.id) {
      // Second click: it really goes.
      const go = el("button", "talk-go", "Move to Trash");
      onClick(go, async () => {
        trashing = null;
        await api.trashConversation(name, talk.id);
        conversations = await api.conversations(name);
      });
      row.append(go);
    } else {
      row.append(el("span", "talk-size", talk.size));
      const bin = el("button", "remove talk-bin");
      bin.type = "button";
      bin.title = "Move to the Trash. Claude Code will no longer list it to resume";
      bin.append(icon("bin", `Move ${talk.title} to the Trash`));
      bin.addEventListener("click", () => {
        trashing = talk.id;
        render(overview);
      });
      row.append(bin);
    }
    list.append(row);
  }
  if (conversations.more > 0) {
    const others = conversations.more === 1 ? "1 smaller one" : `${conversations.more} smaller ones`;
    list.append(el("li", "talks-note", `And ${others}: cprofile clean ${name}`));
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
): HTMLButtonElement {
  const toggle = el("button", "fold");
  toggle.type = "button";
  toggle.dataset.fold = id;
  toggle.title = hint;
  toggle.setAttribute("aria-expanded", String(open));
  toggle.append(icon("chevron"), el("span", "fold-title", label));
  if (!open) toggle.append(el("span", "fold-sum", sum));
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
const KEEP = [
  { days: 30, label: "30 days" },
  { days: 90, label: "90 days" },
  { days: 365, label: "1 year" },
];

/** The exact number of days, to read and to type. Enter, or leaving the
 *  field, applies it. */
function daysField(profile: Profile): HTMLElement {
  const field = el("label", "days-field");
  field.title = "Number of days, 1 or more";
  const input = el("input", "days-input");
  input.type = "number";
  input.min = "1";
  input.step = "1";
  input.inputMode = "numeric";
  input.value = String(profile.keepsDays);
  input.setAttribute("aria-label", `Days ${profile.name} keeps its conversations`);
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
  field.append(input, el("span", "", "d"));
  return field;
}

/** How long each profile keeps its conversations, with a way to change it. */
function historySection(overview: Overview): HTMLElement {
  const section = el("section", "storage-section");
  const kept = new Set(overview.profiles.map((profile) => profile.keeps));
  const sum = kept.size === 1 ? overview.profiles[0].keeps : "Varies";
  section.append(
    foldLine(
      "history",
      "Conversations kept",
      "Claude Code deletes older conversations by itself, without a message",
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
    const row = el("li", "storage-row");
    row.style.setProperty("--hue", hue(profile.name));
    row.append(el("span", "storage-name", profile.name));
    if (profile.keepsDays === null) {
      // Its settings file cannot be read: say so, and offer nothing.
      const unknown = el("span", "keep-unknown", "Settings file unreadable");
      unknown.title = profile.keeps;
      row.append(unknown);
    } else {
      const choices = el("span", "segments");
      choices.setAttribute("role", "group");
      choices.setAttribute("aria-label", `How long ${profile.name} keeps its conversations`);
      for (const choice of KEEP) {
        const button = el("button", "segment", choice.label);
        const current = profile.keepsDays === choice.days;
        button.setAttribute("aria-pressed", String(current));
        if (current) {
          button.type = "button";
        } else {
          onClick(button, () => api.setRetention(profile.name, choice.days));
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

/** A line that unfolds to one line per profile: a bar to scale, then the
 *  size. Folded, it shows the sum. */
function storageSection(measured: StorageOverview, overview: Overview): HTMLElement {
  const sizes = measured.profiles.filter((size) => hues.has(size.name));
  const section = el("section", "storage-section");
  section.append(
    foldLine(
      "storage",
      "Storage",
      "Disk space taken by each profile's Claude Code folder",
      storageOpen,
      measured.total,
      () => {
        storageOpen = !storageOpen;
        render(overview);
      },
    ),
  );
  if (!storageOpen) return section;

  const list = el("ul", "storage");
  // The biggest profile fills the bar; the others are drawn against it.
  const biggest = Math.max(...sizes.map((size) => size.total), 1);
  for (const size of sizes) {
    const item = el("li", "storage-item");
    item.style.setProperty("--hue", hue(size.name));
    // The line is a button: it unfolds the profile's biggest conversations.
    const picked = pickedProfile === size.name;
    const row = el("button", "storage-row storage-pick");
    row.type = "button";
    row.setAttribute("aria-expanded", String(picked));
    row.title = `${size.detail || "Nothing stored yet"}\nClick to see its biggest conversations`;
    row.addEventListener("click", () => {
      pickedProfile = picked ? null : size.name;
      conversations = null;
      trashing = null;
      render(overview);
      void loadConversations(overview);
    });
    const bar = el("span", "bar");
    const fill = el("span", "bar-fill");
    // A profile with anything at all keeps a visible sliver.
    const share = size.total > 0 ? Math.max(size.total / biggest, 0.02) : 0;
    fill.style.width = `${(share * 100).toFixed(1)}%`;
    bar.append(fill);
    row.append(
      el("span", "storage-name", size.name),
      bar,
      el("span", "storage-size", size.total > 0 ? size.size : "Empty"),
    );
    item.append(row);
    if (picked) item.append(conversationList(size.name, overview));
    list.append(item);
  }
  section.append(list);
  return section;
}

// The last reading of what each profile consumed. Like the sizes, it
// arrives after the list and stays on screen until the next one is in.
let usage: UsageOverview | null = null;
let usageOpen = false;
// The profile whose projects and conversations are unfolded.
let usagePicked: string | null = null;

/** What one profile was used for: its projects against the rules, then
 *  the conversations that send the most with each answer. */
function usageDetail(used: Usage): HTMLElement {
  const list = el("ul", "talks");
  if (used.projects.length === 0) {
    list.append(el("li", "talks-note", "No conversation in this period."));
    return list;
  }
  list.append(el("li", "talks-note", "Projects, against today's rules"));
  for (const project of used.projects) {
    const row = el("li", "talk used");
    row.title = `${project.folder}\n${project.why}`;
    row.append(el("span", "used-share", project.share), el("span", "used-folder", project.folder));
    if (project.note) {
      row.append(el("span", project.flagged ? "used-note used-flag" : "used-note", project.note));
    } else {
      // A rule covers it: nothing to read, a quiet mark is enough.
      const ok = el("span", "used-note used-ok", "✓");
      ok.setAttribute("aria-label", "Covered by a rule");
      row.append(ok);
    }
    list.append(row);
  }
  if (used.moreProjects > 0) {
    const others = used.moreProjects === 1 ? "1 lighter one" : `${used.moreProjects} lighter ones`;
    list.append(el("li", "talks-note", `And ${others}: cprofile usage ${used.name}`));
  }

  const heading = el("li", "talks-note", "Conversations, by tokens sent with each answer");
  heading.title = `${used.reread}.\nClaude reads the whole conversation again each time it answers`;
  list.append(heading);
  for (const talk of used.talks) {
    const row = el("li", "talk");
    const text = el("span", "talk-text");
    text.append(el("span", "talk-title", talk.title), el("span", "talk-detail", talk.detail));
    const context = el("span", talk.heavy ? "talk-size used-flag" : "talk-size", talk.context);
    row.title = talk.heavy
      ? `${talk.title}\nSends ${talk.context} tokens with each answer. /compact inside it, or a new conversation when the subject changes, lightens it`
      : `${talk.title}\nSends ${talk.context} tokens with each answer`;
    row.append(text, context);
    list.append(row);
  }
  return list;
}

/** A line that unfolds to one line per profile: its part of the usage of
 *  the last days. Folded, it says how many projects deserve a look. */
function usageSection(read: UsageOverview, overview: Overview): HTMLElement {
  const profiles = read.profiles.filter((used) => hues.has(used.name));
  const flagged = profiles.reduce((sum, used) => sum + used.flagged, 0);
  const section = el("section", "storage-section");
  const period = `Last ${read.days} days`;
  section.append(
    foldLine(
      "usage",
      "Usage",
      `Which projects used each account over the ${period.toLowerCase()}.\nAn estimate from the conversations kept on this Mac, not your plan's limits`,
      usageOpen,
      flagged > 0 ? `${flagged} to check` : period,
      () => {
        usageOpen = !usageOpen;
        render(overview);
      },
    ),
  );
  if (!usageOpen) return section;

  const list = el("ul", "storage");
  const heaviest = Math.max(...profiles.map((used) => used.weight), 1e-9);
  for (const used of profiles) {
    const item = el("li", "storage-item");
    item.style.setProperty("--hue", hue(used.name));
    const picked = usagePicked === used.name;
    const row = el("button", "storage-row storage-pick");
    row.type = "button";
    row.setAttribute("aria-expanded", String(picked));
    row.title =
      used.flagged > 0
        ? `${used.flagged} project(s) to check\nClick to see its projects and conversations`
        : "Click to see its projects and conversations";
    row.addEventListener("click", () => {
      usagePicked = picked ? null : used.name;
      render(overview);
    });
    const bar = el("span", "bar");
    const fill = el("span", "bar-fill");
    const share = used.weight > 0 ? Math.max(used.weight / heaviest, 0.02) : 0;
    fill.style.width = `${(share * 100).toFixed(1)}%`;
    bar.append(fill);
    row.append(
      el("span", "storage-name", used.name),
      bar,
      el("span", "storage-size", used.share || "Idle"),
    );
    item.append(row);
    if (picked) item.append(usageDetail(used));
    list.append(item);
  }
  section.append(list);
  return section;
}

function menu(): HTMLElement {
  const footer = el("footer");
  footer.append(
    menuItem("Edit config file…", "⌘,", () => api.openConfig()),
    menuItem("Quit Claude Profiles", "⌘Q", () => api.quit()),
    el("p", "legal", "Not affiliated with Anthropic"),
  );
  return footer;
}

function notice(text: string, kind = ""): HTMLElement {
  const node = el("p", `notice ${kind}`.trim(), text);
  if (kind === "error") node.setAttribute("role", "alert");
  return node;
}

function render(overview: Overview | null, loadError = ""): void {
  const page = el("div", "page");
  page.append(header());

  if (loadError) page.append(notice(loadError, "error"));
  if (errorText) page.append(notice(errorText, "error"));

  if (overview) {
    assignHues(overview.profiles);
    if (!overview.shimInstalled) {
      page.append(notice("The claude shim is not installed. Run “cprofile setup” in a terminal."));
    }
    const main = el("main");
    if (overview.profiles.length === 0 && !adding) {
      main.append(el("p", "empty", "No profile yet. A profile is one Claude account."));
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
      const title = el("h2", "", "Rules");
      title.title = "Checked from top to bottom. A rule sends a folder or a repository to a profile";
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
    page.append(main);
  }

  page.append(menu());
  root.replaceChildren(page);
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
  // Measuring reads whole folders: the list above does not wait for it.
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

void refresh();
