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
// at a glance in the list and in the rules. Colors are dealt in list order,
// which guarantees that two profiles never share one (up to six profiles).
const HUES = ["#d99a3d", "#2fa39b", "#6f83e6", "#d6648b", "#63a852", "#9a78d8"];
const NO_PROFILE = "#8e8e93";
let hues = new Map<string, string>();

function assignHues(profiles: Profile[]): void {
  hues = new Map(profiles.map((profile, index) => [profile.name, HUES[index % HUES.length]]));
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
    if (overview.profiles.length === 0) {
      main.append(el("p", "empty", "No profile yet. Run “cprofile add” in a terminal to create one."));
    } else {
      const list = el("ul", "profiles");
      list.append(...overview.profiles.map(profileRow));
      main.append(list);
    }
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
  if (event.key === "Escape") void api.hide();
});

void refresh();
