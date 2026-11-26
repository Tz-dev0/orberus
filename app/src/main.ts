// The popover under the menu bar icon: profiles, rules, and a few actions.
// Plain TypeScript and DOM calls: the page is one small list, a framework
// would add more code than it removes.

import { api, insideApp, type Overview, type Profile, type Rule } from "./api";

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

function profileRow(profile: Profile): HTMLElement {
  const row = el("li", "profile");
  row.style.setProperty("--hue", hue(profile.name));
  if (!profile.account) row.classList.add("signed-out");

  // The whole left part is the main action: start Claude Code there.
  const open = el("button", "profile-open");
  open.append(el("span", "seal", profile.name.slice(0, 1).toUpperCase()));
  const who = el("span", "who");
  who.append(
    el("span", "name", profile.name),
    el("span", "account", profile.account ?? "Not signed in"),
  );
  open.append(who, el("span", "go", profile.account ? "Open" : "Sign in"));
  open.title = profile.account
    ? `Open Claude Code as ${profile.account} in a terminal`
    : "Open Claude Code in a terminal to sign in";
  onClick(open, () => api.openClaude(profile.name));
  row.append(open);

  if (profile.isDefault) {
    const tag = el("span", "default-tag", "Default");
    tag.title = "Used where no rule applies";
    row.append(tag);
  } else {
    const set = el("button", "set-default", "Set as default");
    onClick(set, () => api.setDefault(profile.name));
    row.append(set);
  }
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

function menu(): HTMLElement {
  const footer = el("footer");
  const edit = el("button", "menu-item", "Edit config file…");
  onClick(edit, () => api.openConfig());
  const quit = el("button", "menu-item", "Quit Claude Profiles");
  onClick(quit, () => api.quit());
  footer.append(edit, quit, el("p", "legal", "Not affiliated with Anthropic"));
  return footer;
}

function notice(text: string, kind = ""): HTMLElement {
  const node = el("p", `notice ${kind}`.trim(), text);
  if (kind === "error") node.setAttribute("role", "alert");
  return node;
}

function render(overview: Overview | null, loadError = ""): void {
  const page = el("div", "page");

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
    list.append(...overview.profiles.map(profileRow), newProfileRow(overview));
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
  try {
    render(await api.overview());
  } catch (error) {
    // The config could not be read: say why instead of showing stale data.
    render(null, String(error));
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
  if (event.key !== "Escape") return;
  // Escape first backs out of the form, then closes the window.
  if (adding) {
    closeForm();
    void refresh();
  } else {
    void api.hide();
  }
});

void refresh();
