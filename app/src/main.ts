// The popover under the menu bar icon: profiles, rules, and a few actions.
// Plain TypeScript and DOM calls: the page is one small list, a framework
// would add more code than it removes.

import { api, insideApp, type Overview, type Profile, type Rule, type StorageOverview } from "./api";

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
  // A seal inside a ring: what the default profile wears.
  ring: "M8 5a3 3 0 1 0 0 6 3 3 0 0 0 0-6zM8 2a6 6 0 1 0 0 12A6 6 0 0 0 8 2z",
  chevron: "M6 3.5 10.5 8 6 12.5",
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
  } else {
    // An icon and not words: with the bin next to it, words would push
    // the account's e-mail out of the row.
    const set = el("button", "set-default");
    set.title = `Use ${profile.name} where no rule applies`;
    set.append(icon("ring", `Set ${profile.name} as default`));
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

function ruleRow(rule: Rule): HTMLElement {
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
    const row = el("li", "storage-row");
    row.style.setProperty("--hue", hue(size.name));
    row.title = size.detail || "Nothing stored yet";
    const bar = el("span", "bar");
    const fill = el("span", "bar-fill");
    // A profile with anything at all keeps a visible sliver.
    const share = size.total > 0 ? Math.max(size.total / biggest, 0.02) : 0;
    fill.style.width = `${(share * 100).toFixed(1)}%`;
    bar.append(fill);
    row.append(el("span", "storage-name", size.name), bar, el("span", "storage-size", size.total > 0 ? size.size : "Empty"));
    list.append(row);
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
    if (overview.rules.length > 0) {
      const section = el("section", "rules-section");
      const title = el("h2", "", "Rules");
      title.title = "Checked from top to bottom";
      const list = el("ul", "rules");
      list.append(...overview.rules.map(ruleRow));
      section.append(title, list);
      main.append(section);
    }
    // Only the profiles still in the list: a measure can be a moment old.
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
    if (changed && !adding && !removing) render(overview);
  } catch {
    // Sizes are a comfort, not worth an error message: keep the last ones.
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
  if (adding || removing) {
    closeForm();
    removing = null;
    void refresh();
  } else {
    void api.hide();
  }
});

void refresh();
