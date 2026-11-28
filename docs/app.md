# Menu bar app

Orberus.app is an optional companion to the CLI. It lives in the menu
bar, has no Dock icon, and reads and writes the same config as `orberus`.

Install it as described in [installation.md](installation.md#install-the-menu-bar-app).
The CLI must be installed too: the app starts Claude Code through its shim.

## Opening it

Click the icon in the menu bar: a small window opens under it and closes when
you click elsewhere, or press Escape. Right-click the icon to quit.

Once opened, the app comes back by itself each time you log in to your Mac.
To stop that, turn off "Start Orberus with my Mac" at the bottom of its
window; the choice is remembered. Behind the switch is one small file,
`~/Library/LaunchAgents/com.github.tzdev0.orberus.plist`, which the app
writes the first time it starts and macOS reads at each login; macOS may
tell you a background item was added. Move the app to another folder and it
puts the file right the next time you open it.

The app only shows and changes things. `claude` picks the account by folder
whether the app is open or not, and quitting it turns nothing off. To stop
Orberus for a while, use the "Pause Orberus" switch at the bottom of the
window, or [`orberus pause`](commands.md#pause-resume): a banner then stays
at the top of the window, with "Resume", until you end the pause.

On a Mac with a notch, macOS can hide the icon when the menu bar is full.
Opening the app again from the Finder or Spotlight shows the window.

## Updates

Once a day, the app looks on GitHub for a release later than itself. When
there is one, a line "Update to Orberus 0.2.0…" appears at the bottom of the
window; it opens [the page that says how to update](installation.md#update).
The app does not replace itself, and the command is updated the same way.

That check is the only time the app uses the network: one request to
`github.com`, for the address of the latest release of this repository.
Nothing about you or your profiles is in it. Without network there is simply
no line. The `claude` shim never does this: it makes no network request.

## What it shows

- **Profiles**: each profile with a coloured dot, filled when an account is
  logged in and hollow when the profile is empty, the account's address, and a badge
  with its subscription (`Max 5x`, `Team`…) when Claude Code recorded it.
  A second ring marks the default profile.
- **Rules**: every rule, with the profile it leads to, and whether it comes
  from your config or from team rules. The list is read from top to bottom
  and ends with "Everywhere else": the default profile, where no rule
  applies. Leave the pointer on a rule to read it in plain words ("In every
  repository of github.com/acme, claude uses work"). Before the first rule,
  a line says what a rule is.
- **Usage** (folded): for each profile, its account's 5-hour and weekly
  limits, as Claude shows them: how much is used, when it starts over, and a
  bar where full is the limit. These are not live numbers: they are the last
  ones noted for that account, by Claude Desktop while it runs or by Claude
  Code after an answer in a terminal, with their age and who noted them,
  rounded to a whole percent, so approximate to about one percent when fresh; "Show 5-hour and weekly limits" or "Add Claude Code" turns Claude Code's
  reports on for a profile (see [`orberus limits`](commands.md#limits)).
  Click a profile to see its heaviest projects of the last 7 days, each
  checked against today's rules, and the conversations that send the most
  tokens with each answer. Folded, the line says how many projects deserve a
  look. The projects are the same estimate as
  [`orberus usage`](commands.md#usage).
- **Conversations kept** (folded): how long each profile keeps its
  conversations.
- **Storage** (folded): the disk space of each profile. Each bar is that
  profile's folder, cut by what fills it: conversations, file history, plugins
  and skills, other.
- **Claude Desktop** (folded, shown when Claude Desktop is installed): one
  line per profile, with whether its Claude Desktop is open. Folded, it says
  how many are open. See [desktop.md](desktop.md).
- **Editors** (folded, shown when VS Code, Cursor or another editor built on
  VS Code is found): whether the Claude Code extension of each one goes
  through Orberus. Folded, it says how many still skip it.
- **Terminal** (folded): the app that "Open…" starts Claude Code in.

The last line of the window gives the app's version. The command has its
own: `orberus --version`.

## What you can do

| Action | How |
|---|---|
| Open Claude Code in a profile | Click the profile. You pick the project folder, then a terminal window opens there |
| Create a profile | "New profile…", or ⌘N |
| Set the default profile | Hover the profile, then "Set as default" |
| Create a rule | "New rule…" under the rules. You pick a folder, and the form says which profile `claude` uses there today and why. Then you choose where the rule applies (every repository of its owner, this repository, or the folder), the profile, and whether to lock it; the rule about to be written is spelled out in plain words, and you are told when something else would still decide in that folder |
| Remove a rule | Hover one of your own rules, then the bin icon and "Remove rule". Team rules cannot be removed |
| Remove a profile | Hover the profile, then the bin icon. The row says what goes and what stays before you confirm |
| Change how long conversations are kept | Unfold "Conversations kept": 30 days, 90 days, 1 year, or a number of days |
| Move big conversations to the Trash | Unfold "Storage", click a profile, then the bin icon of a conversation and "Move to Trash" |
| Open Claude Desktop with a profile's account | Unfold "Claude Desktop" and click "Open" next to the profile: the same as [`orberus desktop <profile>`](desktop.md). When it is already open, the button says "Show" and brings it to the front. The first time, you sign in there |
| Pick the account by folder in an editor | Unfold "Editors" and click "Turn on" next to it: the same as [`orberus ide on`](editors.md) for that editor. "Turn off" undoes it. Reload the editor's window afterwards |
| Pause Orberus | Turn on "Pause Orberus" at the bottom of the window: `claude` starts as if Orberus was not installed, rules included. "Resume" in the banner, or the switch again, ends it |
| Update | When "Update to Orberus …" shows at the bottom of the window, click it: it opens the page that says how |
| Stop the app from opening when you log in | Turn off "Start Orberus with my Mac" at the bottom of the window. Click it again to turn it back on |
| Choose the terminal app | Unfold "Terminal" and click one. "System default" is the app macOS opens `.command` files with, usually Terminal |
| Open the config file | ⌘, |
| Quit | ⌘Q |

Opening Claude Code from the app goes through the shim, so a locked rule on
the folder you pick still has the last word.

Removing a profile works as `orberus remove` does: the profile's Claude Code
folder is kept unless you tick "Also delete its folder and sign out", which is
`orberus remove --purge`. That also deletes the data of the profile's own
Claude Desktop, and is refused while that Claude Desktop is open.

## Language

The window follows the language of macOS: French when French comes first
among your preferred languages, English otherwise. Sizes, percentages and
dates are written the way that language writes them (`3,4 Go`, `63 %`,
`il y a 2 jours`).

Two things stay in English: the message of an error, and the `orberus`
command line.

## What it does not do yet

- A rule that needs both a folder and a git remote, or a pattern typed by
  hand, is created with `orberus rule add` or in `config.toml`.
- The terminal list holds the installed apps that declare they run script
  files (Terminal, and others such as Ghostty, WezTerm or kitty). Opening
  Claude Code was tested with Terminal only. An app that does not accept a
  script file, such as Alacritty, is not listed.
- Every Claude Desktop is named "Claude" in the Dock, whatever its profile.
  Signing in to a second one was not tried yet: see
  [desktop.md](desktop.md#what-was-tried).
- Error messages are in English, whatever the language of the window.
- It is not signed with an Apple developer certificate. See
  [installation.md](installation.md#why-curl-and-not-the-browser).
