// Every word the window shows, in each language, and how numbers, sizes and
// ages are written. The Rust side sends data (bytes, seconds, fractions,
// short codes); the sentences are made here.
//
// To add a language: one more object shaped like `en` (TypeScript refuses a
// missing or misspelled key), and one more line in `LANGUAGES`.
// No dependency: the browser already knows how each language writes a
// number or "3 days ago" (`Intl`).

const en = {
  appName: "Orberus",

  // Profiles
  open: "Open…",
  signIn: "Sign in…",
  notSignedIn: "Not signed in",
  planHint: "This account's subscription: {plan}",
  openHint: "Choose a project folder, then open Claude Code there as {account}",
  signInHint: "Choose a project folder, then open Claude Code there to sign in",
  pickOpenTitle: "Open Claude Code as {name} in",
  default: "Default",
  defaultHint: "Used where no rule applies",
  setDefault: "Set as default",
  setDefaultHint: "Use {name} where no rule applies",
  removeProfileHint: "Remove {name}…",
  removeProfileLabel: "Remove {name}",
  removeAsk: "Remove {name}?",
  ownRuleOne: "Your rule for it will be removed too.",
  ownRuleMany: "Your {n} rules for it will be removed too.",
  keptFolder: "Its Claude Code folder stays on this Mac, with its login and history.",
  keptNative: "Its folder {location} is your own Claude Code setup: it stays on this Mac.",
  deletedFolder:
    "{location} will be deleted with its history and settings, and its login removed from the keychain. This cannot be undone.",
  alsoDelete: "Also delete its folder and sign out",
  teamNeedsOne:
    "1 rule from your team needs this profile: Claude Code will refuse to start where it applies.",
  teamNeedsMany:
    "{n} rules from your team need this profile: Claude Code will refuse to start where they apply.",
  cancel: "Cancel",
  removeAndDelete: "Remove and delete",
  removeProfile: "Remove profile",
  newProfile: "New profile…",
  namePlaceholder: "Name, for example work",
  nameLabel: "Profile name",
  reuseLogin: "Use the Claude Code login already on this Mac",
  createProfile: "Create profile",
  noProfile: "No profile yet. A profile is one Claude account.",

  // Rules
  rules: "Rules",
  rulesHint: "Checked from top to bottom. A rule sends a folder or a repository to a profile",
  newRule: "New rule…",
  newRuleHint: "Choose a folder, then where the rule applies and which profile it uses",
  pickRuleTitle: "Folder the rule is about",
  ruleWhere: "Where the rule applies",
  choiceOwner: "Every repository of {owner}",
  choiceRepo: "Only the repository {repo}",
  choiceFolder: "The folder {path} and everything below",
  ruleProfile: "Profile to use there",
  lockHint:
    "A locked rule wins over everything: Claude Code does not start there with another profile",
  lockIt: "Lock it: refuse any other profile there",
  addRule: "Add rule",
  gitRemote: "Git remote",
  folder: "Folder",
  team: "Team",
  teamHint: "Provided by your company in team.toml",
  locked: "Locked",
  lockedHint: "Locked: no other profile can be used there",
  removeRule: "Remove rule",
  removeThisRule: "Remove this rule",

  // Conversations
  reading: "Reading…",
  noConversation: "No conversation on disk.",
  inUse: "In use?",
  inUseHint: "Changed in the last hour: it may still be open, so it stays",
  moveToTrash: "Move to Trash",
  trashHint: "Move to the Trash. Claude Code will no longer list it to resume",
  trashLabel: "Move {title} to the Trash",
  smallerOne: "And 1 smaller one: orberus clean {name}",
  smallerMany: "And {n} smaller ones: orberus clean {name}",
  untitled: "Untitled ({id})",
  justNow: "just now",

  // Conversations kept
  kept: "Conversations kept",
  keptHint: "Claude Code deletes older conversations by itself, without a message",
  varies: "Varies",
  dayOne: "1 day",
  dayMany: "{n} days",
  year: "1 year",
  daysFieldHint: "Number of days, 1 or more",
  daysFieldLabel: "Days {name} keeps its conversations",
  daysUnit: "d",
  keepGroup: "How long {name} keeps its conversations",
  settingsUnreadable: "Settings file unreadable",

  // Storage
  storage: "Storage",
  storageHint: "Disk space taken by each profile's Claude Code folder",
  nothingStored: "Nothing stored yet",
  storageClick: "Click to see its biggest conversations",
  empty: "Empty",
  partConversations: "conversations",
  partFileHistory: "file history",
  partExtensions: "plugins and skills",
  partOther: "other",
  legendConversations: "Conversations",
  legendFileHistory: "File history",
  legendExtensions: "Plugins and skills",
  legendOther: "Other",

  // Usage
  usage: "Usage",
  usageHint:
    "Which projects used each account over the last {n} days.\nAn estimate from the conversations kept on this Mac, not your plan's limits",
  lastDays: "Last {n} days",
  toCheck: "{n} to check",
  projectsToCheckOne: "1 project to check",
  projectsToCheckMany: "{n} projects to check",
  usageClick: "Click to see its projects and conversations",
  idle: "Idle",
  shareHint: "Its part of the usage of all profiles over the last {n} days",
  shareFact: "{share} · {projects}",
  projectsOne: "1 project",
  limit5h: "5-hour limit",
  limitWeek: "Weekly limit",
  limitFact: "{used} · resets {when}",
  limitFresh: "{used} · started over",
  limitsUpdated: "{source}, {when}",
  limitsHint:
    "The last numbers Claude Code or Claude Desktop noted for this account, not a live reading.\nRounded to a whole percent: approximate to about 1% when fresh",
  limitsWaiting: "Limits appear after the next answer in Claude Code",
  limitsShow: "Show 5-hour and weekly limits",
  limitsShowHint:
    "Adds a status line to this profile's Claude Code settings: Claude Code then reports this account's limits here, with their reset times, even when Claude Desktop is closed or logged in to another account.\nA status line of your own is kept",
  limitsShowToo: "Add Claude Code",
  limitsHide: "Turn off",
  limitsHideHint: "Removes the status line from this profile's Claude Code settings",
  projectsMany: "{n} projects",
  noActivity: "No conversation in this period.",
  projectsHeading: "Projects, against today's rules",
  covered: "Covered by a rule",
  noteDefault: "no rule",
  noteElsewhere: "would use {profile}",
  noteGone: "folder gone",
  noteUnknown: "not checked",
  whyCovered: "Sent to {name} by {reason}",
  whyDefault: "No rule names this project: it uses {name} only as the default",
  whyElsewhere:
    "Started there today, claude would use {profile} ({reason}). It ran with {name} by an explicit choice, or the rules changed since",
  whyGone: "The folder was deleted, so its rules cannot be checked",
  reasonLocked: "the locked rule {rule} in {source}",
  reasonRule: "the rule {rule} in {source}",
  reasonFile: "the file {file}",
  reasonExplicit: "an explicit choice",
  reasonDefault: "the default profile",
  lighterOne: "And 1 lighter one: orberus usage {name}",
  lighterMany: "And {n} lighter ones: orberus usage {name}",
  talksHeading: "Conversations, by tokens sent with each answer",
  rereadHint:
    "About {share} went to reading context again.\nClaude reads the whole conversation again each time it answers",
  talkDetail: "{folder}, {share} of this profile",
  talkSends: "Sends {context} tokens with each answer",
  talkHeavy:
    "Sends {context} tokens with each answer. /compact inside it, or a new conversation when the subject changes, lightens it",

  // Editors
  editors: "Editors",
  editorsHint:
    "VS Code, Cursor and the editors built on VS Code: their Claude Code extension starts its own copy of Claude Code, without Orberus, unless it is sent through it here.\nJetBrains IDEs and terminals already go through Orberus",
  editorsSkipOne: "1 skips Orberus",
  editorsSkipMany: "{n} skip Orberus",
  editorsAllOn: "Through Orberus",
  editorsNone: "No extension",
  editorOn: "through Orberus",
  editorOnHint:
    "In this editor, Claude Code gets the account of the folder that is open, by the same rules as in a terminal.\n{file}",
  editorOff: "skips Orberus",
  editorOffHint: "In this editor, Claude Code always uses the same account, whatever folder is open.\n{file}",
  editorAbsent: "extension not installed",
  editorAbsentHint: "The Claude Code extension is not installed in this editor: nothing to do.\n{file}",
  editorOther: "through {program}",
  editorOtherHint: "This editor already starts Claude Code through another program. It is not replaced.\n{file}",
  editorUnreadable: "settings not readable",
  editorUnreadableHint: "{why}\n{file}",
  editorTurnOn: "Turn on",
  editorTurnOnHint: "Writes one setting in this editor's settings file. The rest of the file stays as it is",
  editorTurnOff: "Turn off",
  editorTurnOffHint: "Removes that setting from this editor's settings file",
  editorsNote: "Reload the editor's window after a change.",

  // Terminal
  terminal: "Terminal",
  terminalHint: "The app a profile's “Open…” button starts Claude Code in",
  systemDefault: "System default",
  systemDefaultHint: "The app macOS opens .command files with: Terminal, unless you changed it",
  lookingTerminals: "Looking for terminal apps…",
  terminalsNote: "Listed: the installed apps that can run a script file.",

  // Menu and notices
  openAtLogin: "Open at login",
  openAtLoginHint:
    "Opens Orberus in the menu bar when you log in to your Mac.\nThe app only shows and changes things: claude picks the account by folder whether the app is open or not",
  editConfig: "Edit config file…",
  quit: "Quit Orberus",
  legal: "Not affiliated with Anthropic",
  shimMissing: "The claude shim is not installed. Run “orberus setup” in a terminal.",

  // Units of size, smallest first.
  unitB: "B",
  unitKB: "KB",
  unitMB: "MB",
  unitGB: "GB",
  unitTB: "TB",
};

/** The name of a text. A typo in `t("…")` does not compile. */
export type Key = keyof typeof en;

// `Record<Key, string>`: exactly the keys of `en`, no more, no less.
const fr: Record<Key, string> = {
  appName: "Orberus",

  open: "Ouvrir…",
  signIn: "Se connecter…",
  notSignedIn: "Non connecté",
  planHint: "Abonnement de ce compte : {plan}",
  openHint: "Choisir un dossier de projet, puis y ouvrir Claude Code avec {account}",
  signInHint: "Choisir un dossier de projet, puis y ouvrir Claude Code pour se connecter",
  pickOpenTitle: "Ouvrir Claude Code avec {name} dans",
  default: "Par défaut",
  defaultHint: "Utilisé là où aucune règle ne s'applique",
  setDefault: "Définir par défaut",
  setDefaultHint: "Utiliser {name} là où aucune règle ne s'applique",
  removeProfileHint: "Supprimer {name}…",
  removeProfileLabel: "Supprimer {name}",
  removeAsk: "Supprimer {name} ?",
  ownRuleOne: "Votre règle qui le vise sera aussi supprimée.",
  ownRuleMany: "Vos {n} règles qui le visent seront aussi supprimées.",
  keptFolder: "Son dossier Claude Code reste sur ce Mac, avec son login et son historique.",
  keptNative:
    "Son dossier {location} est votre propre installation de Claude Code : il reste sur ce Mac.",
  deletedFolder:
    "{location} sera supprimé avec son historique et ses réglages, et son login retiré du trousseau. C'est irréversible.",
  alsoDelete: "Supprimer aussi son dossier et se déconnecter",
  teamNeedsOne:
    "1 règle de votre équipe a besoin de ce profil : Claude Code refusera de démarrer là où elle s'applique.",
  teamNeedsMany:
    "{n} règles de votre équipe ont besoin de ce profil : Claude Code refusera de démarrer là où elles s'appliquent.",
  cancel: "Annuler",
  removeAndDelete: "Supprimer et effacer",
  removeProfile: "Supprimer le profil",
  newProfile: "Nouveau profil…",
  namePlaceholder: "Nom, par exemple work",
  nameLabel: "Nom du profil",
  reuseLogin: "Utiliser le login Claude Code déjà présent sur ce Mac",
  createProfile: "Créer le profil",
  noProfile: "Aucun profil pour l'instant. Un profil correspond à un compte Claude.",

  rules: "Règles",
  rulesHint: "Lues de haut en bas. Une règle envoie un dossier ou un dépôt vers un profil",
  newRule: "Nouvelle règle…",
  newRuleHint: "Choisir un dossier, puis où la règle s'applique et quel profil elle utilise",
  pickRuleTitle: "Dossier concerné par la règle",
  ruleWhere: "Où la règle s'applique",
  choiceOwner: "Tous les dépôts de {owner}",
  choiceRepo: "Seulement le dépôt {repo}",
  choiceFolder: "Le dossier {path} et tout ce qu'il contient",
  ruleProfile: "Profil à y utiliser",
  lockHint:
    "Une règle verrouillée passe avant tout : Claude Code n'y démarre pas avec un autre profil",
  lockIt: "Verrouiller : y refuser tout autre profil",
  addRule: "Ajouter la règle",
  gitRemote: "Dépôt git distant",
  folder: "Dossier",
  team: "Équipe",
  teamHint: "Fournie par votre entreprise dans team.toml",
  locked: "Verrouillée",
  lockedHint: "Verrouillée : aucun autre profil ne peut y être utilisé",
  removeRule: "Supprimer la règle",
  removeThisRule: "Supprimer cette règle",

  reading: "Lecture…",
  noConversation: "Aucune conversation sur le disque.",
  inUse: "En cours ?",
  inUseHint:
    "Modifiée depuis moins d'une heure : elle est peut-être encore ouverte, donc elle reste",
  moveToTrash: "Mettre à la Corbeille",
  trashHint: "Mettre à la Corbeille. Claude Code ne la proposera plus à la reprise",
  trashLabel: "Mettre {title} à la Corbeille",
  smallerOne: "Et 1 plus petite : orberus clean {name}",
  smallerMany: "Et {n} plus petites : orberus clean {name}",
  untitled: "Sans titre ({id})",
  justNow: "à l'instant",

  kept: "Conversations gardées",
  keptHint: "Claude Code supprime de lui-même les conversations plus anciennes, sans prévenir",
  varies: "Variable",
  dayOne: "1 jour",
  dayMany: "{n} jours",
  year: "1 an",
  daysFieldHint: "Nombre de jours, 1 ou plus",
  daysFieldLabel: "Jours pendant lesquels {name} garde ses conversations",
  daysUnit: "j",
  keepGroup: "Durée pendant laquelle {name} garde ses conversations",
  settingsUnreadable: "Fichier de réglages illisible",

  storage: "Stockage",
  storageHint: "Espace disque occupé par le dossier Claude Code de chaque profil",
  nothingStored: "Rien de stocké pour l'instant",
  storageClick: "Cliquer pour voir ses plus grosses conversations",
  empty: "Vide",
  partConversations: "de conversations",
  partFileHistory: "d'historique de fichiers",
  partExtensions: "de plugins et skills",
  partOther: "d'autres fichiers",
  legendConversations: "Conversations",
  legendFileHistory: "Historique de fichiers",
  legendExtensions: "Plugins et skills",
  legendOther: "Autres",

  usage: "Usage",
  usageHint:
    "Quels projets ont utilisé chaque compte sur les {n} derniers jours.\nUne estimation tirée des conversations gardées sur ce Mac, pas les limites de votre abonnement",
  lastDays: "{n} derniers jours",
  toCheck: "{n} à vérifier",
  projectsToCheckOne: "1 projet à vérifier",
  projectsToCheckMany: "{n} projets à vérifier",
  usageClick: "Cliquer pour voir ses projets et conversations",
  idle: "Inactif",
  shareHint: "Sa part de l'usage de tous les profils sur les {n} derniers jours",
  shareFact: "{share} · {projects}",
  projectsOne: "1 projet",
  limit5h: "Limite de 5 h",
  limitWeek: "Limite hebdomadaire",
  limitFact: "{used} · remise à zéro {when}",
  limitFresh: "{used} · repartie de zéro",
  limitsUpdated: "{source}, {when}",
  limitsHint:
    "Les derniers chiffres notés par Claude Code ou Claude Desktop pour ce compte, pas une mesure en direct.\nArrondis au pourcent : approximatifs à environ 1 % près quand ils sont récents",
  limitsWaiting: "Les limites apparaîtront après la prochaine réponse dans Claude Code",
  limitsShow: "Afficher les limites de 5 h et hebdomadaire",
  limitsShowHint:
    "Ajoute une ligne d'état aux réglages Claude Code de ce profil : Claude Code indique alors ici les limites de ce compte, avec leur heure de remise à zéro, même quand Claude Desktop est fermé ou connecté à un autre compte.\nVotre propre ligne d'état est conservée",
  limitsShowToo: "Ajouter Claude Code",
  limitsHide: "Désactiver",
  limitsHideHint: "Retire la ligne d'état des réglages Claude Code de ce profil",
  projectsMany: "{n} projets",
  noActivity: "Aucune conversation sur cette période.",
  projectsHeading: "Projets, selon les règles d'aujourd'hui",
  covered: "Couvert par une règle",
  noteDefault: "aucune règle",
  noteElsewhere: "utiliserait {profile}",
  noteGone: "dossier supprimé",
  noteUnknown: "non vérifié",
  whyCovered: "Envoyé vers {name} par {reason}",
  whyDefault:
    "Aucune règle ne nomme ce projet : il utilise {name} seulement parce que c'est le profil par défaut",
  whyElsewhere:
    "Lancé là aujourd'hui, claude utiliserait {profile} ({reason}). Il a tourné avec {name} par un choix explicite, ou les règles ont changé depuis",
  whyGone: "Le dossier a été supprimé, ses règles ne peuvent donc pas être vérifiées",
  reasonLocked: "la règle verrouillée {rule} de {source}",
  reasonRule: "la règle {rule} de {source}",
  reasonFile: "le fichier {file}",
  reasonExplicit: "un choix explicite",
  reasonDefault: "le profil par défaut",
  lighterOne: "Et 1 plus léger : orberus usage {name}",
  lighterMany: "Et {n} plus légers : orberus usage {name}",
  talksHeading: "Conversations, par tokens envoyés à chaque réponse",
  rereadHint:
    "Environ {share} est parti en relecture du contexte.\nClaude relit toute la conversation à chaque réponse",
  talkDetail: "{folder}, {share} de ce profil",
  talkSends: "Envoie {context} tokens à chaque réponse",
  talkHeavy:
    "Envoie {context} tokens à chaque réponse. /compact dans la conversation, ou une nouvelle conversation quand le sujet change, l'allège",

  editors: "Éditeurs",
  editorsHint:
    "VS Code, Cursor et les éditeurs bâtis sur VS Code : leur extension Claude Code lance sa propre copie de Claude Code, sans Orberus, sauf si vous la faites passer par lui ici.\nLes IDE JetBrains et les terminaux passent déjà par Orberus",
  editorsSkipOne: "1 sans Orberus",
  editorsSkipMany: "{n} sans Orberus",
  editorsAllOn: "Par Orberus",
  editorsNone: "Aucune extension",
  editorOn: "passe par Orberus",
  editorOnHint:
    "Dans cet éditeur, Claude Code prend le compte du dossier ouvert, selon les mêmes règles que dans un terminal.\n{file}",
  editorOff: "sans Orberus",
  editorOffHint:
    "Dans cet éditeur, Claude Code utilise toujours le même compte, quel que soit le dossier ouvert.\n{file}",
  editorAbsent: "extension non installée",
  editorAbsentHint: "L'extension Claude Code n'est pas installée dans cet éditeur : rien à faire.\n{file}",
  editorOther: "passe par {program}",
  editorOtherHint: "Cet éditeur lance déjà Claude Code par un autre programme. Il n'est pas remplacé.\n{file}",
  editorUnreadable: "réglages illisibles",
  editorUnreadableHint: "{why}\n{file}",
  editorTurnOn: "Activer",
  editorTurnOnHint: "Écrit un réglage dans le fichier de réglages de cet éditeur. Le reste du fichier ne change pas",
  editorTurnOff: "Désactiver",
  editorTurnOffHint: "Retire ce réglage du fichier de réglages de cet éditeur",
  editorsNote: "Rechargez la fenêtre de l'éditeur après un changement.",

  terminal: "Terminal",
  terminalHint: "L'app dans laquelle le bouton « Ouvrir… » d'un profil lance Claude Code",
  systemDefault: "Choix du système",
  systemDefaultHint:
    "L'app avec laquelle macOS ouvre les fichiers .command : Terminal, sauf si vous l'avez changé",
  lookingTerminals: "Recherche des apps de terminal…",
  terminalsNote: "Listées : les apps installées qui savent lancer un fichier de script.",

  openAtLogin: "Ouvrir à la connexion",
  openAtLoginHint:
    "Ouvre Orberus dans la barre de menus quand vous ouvrez votre session.\nL'app ne fait qu'afficher et régler : claude choisit le compte selon le dossier, que l'app soit ouverte ou non",
  editConfig: "Modifier le fichier de config…",
  quit: "Quitter Orberus",
  legal: "Sans lien avec Anthropic",
  shimMissing: "Le shim claude n'est pas installé. Lancez « orberus setup » dans un terminal.",

  // As the Finder writes them in French.
  unitB: "o",
  unitKB: "ko",
  unitMB: "Mo",
  unitGB: "Go",
  unitTB: "To",
};

const LANGUAGES = { en, fr };
export type Language = keyof typeof LANGUAGES;

/** The language of the window: the one the system asks for when we have
 *  it, English otherwise. `?lang=fr` in the address forces one, to look at
 *  a language in the browser preview. */
function pick(): Language {
  const known = (code: string | null | undefined): Language | null => {
    const short = (code ?? "").slice(0, 2).toLowerCase();
    return short in LANGUAGES ? (short as Language) : null;
  };
  const forced = known(new URLSearchParams(window.location.search).get("lang"));
  if (forced) return forced;
  // The system's languages, in the user's order of preference.
  for (const code of navigator.languages ?? [navigator.language]) {
    const found = known(code);
    if (found) return found;
  }
  return "en";
}

export const language: Language = pick();
const texts: Record<Key, string> = LANGUAGES[language];

/** A text in the window's language. `{name}` in it is replaced by
 *  `values.name`. */
export function t(key: Key, values: Record<string, string | number> = {}): string {
  return texts[key].replace(/\{(\w+)\}/g, (whole, name: string) =>
    name in values ? String(values[name]) : whole,
  );
}

/** One of two texts by count: `one` for exactly 1, `many` otherwise, with
 *  `{n}` available in both. English and French agree on this split, apart
 *  from 0, which is never shown here. */
export function count(n: number, one: Key, many: Key, values: Record<string, string | number> = {}): string {
  return t(n === 1 ? one : many, { n, ...values });
}

/** A number the way the language writes it: "3.4" or "3,4". */
function number(value: number, decimals: number): string {
  return new Intl.NumberFormat(language, {
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  }).format(value);
}

/** A size on disk, in decimal units like the Finder: "3.4 GB", "3,4 Go".
 *  Same rounding as `orberus disk`. */
export function bytes(size: number): string {
  if (size < 1000) return `${size} ${t("unitB")}`;
  const units: Key[] = ["unitKB", "unitMB", "unitGB", "unitTB"];
  let value = size / 1000;
  let unit = 0;
  while (value >= 999.5 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }
  // One decimal where it says something: small numbers, and gigabytes.
  const decimals = value < 9.95 || (unit >= 2 && value < 99.95) ? 1 : 0;
  return `${number(value, decimals)} ${t(units[unit])}`;
}

/** A share from 0 to 1: "63%" or "63 %". A small one that is not nothing
 *  reads "<1%" instead of a misleading "0%". */
export function percent(share: number): string {
  const format = new Intl.NumberFormat(language, { style: "percent", maximumFractionDigits: 0 });
  if (share > 0 && share < 0.01) return `<${format.format(0.01)}`;
  return format.format(share);
}

/** A number of tokens, short: "850", "482k", "1.2M". */
export function tokens(amount: number): string {
  if (amount < 1000) return String(amount);
  if (amount < 999_500) return `${Math.round(amount / 1000)}k`;
  return `${number(amount / 1_000_000, 1)}M`;
}

/** How long ago, from a number of seconds: "3 days ago", "il y a 3 jours". */
export function ago(seconds: number): string {
  if (seconds < 60) return t("justNow");
  const format = new Intl.RelativeTimeFormat(language, { numeric: "always" });
  if (seconds < 3600) return format.format(-Math.floor(seconds / 60), "minute");
  if (seconds < 86_400) return format.format(-Math.floor(seconds / 3600), "hour");
  return format.format(-Math.floor(seconds / 86_400), "day");
}

/** In how long, from a number of seconds: "in 4 hours", "dans 4 heures". */
export function until(seconds: number): string {
  const format = new Intl.RelativeTimeFormat(language, { numeric: "always" });
  if (seconds < 3600) return format.format(Math.max(1, Math.floor(seconds / 60)), "minute");
  if (seconds < 86_400) return format.format(Math.floor(seconds / 3600), "hour");
  return format.format(Math.floor(seconds / 86_400), "day");
}

/** How long conversations are kept: "30 days", "1 year". */
export function days(amount: number): string {
  if (amount === 365) return t("year");
  return count(amount, "dayOne", "dayMany");
}
