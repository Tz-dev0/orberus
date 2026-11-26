# Menu bar app

Claude Profiles.app is an optional companion to the CLI. It lives in the menu
bar, has no Dock icon, and reads and writes the same config as `cprofile`.

Install it as described in [installation.md](installation.md#install-the-menu-bar-app).
The CLI must be installed too: the app starts Claude Code through its shim.

## Opening it

Click the icon in the menu bar: a small window opens under it and closes when
you click elsewhere, or press Escape. Right-click the icon to quit.

On a Mac with a notch, macOS can hide the icon when the menu bar is full.
Opening the app again from the Finder or Spotlight shows the window.

## What it shows

- **Profiles**: each profile with a coloured dot, filled when an account is
  logged in and hollow when the profile is empty, and the account's address.
  A second ring marks the default profile.
- **Rules**: every rule, with the profile it leads to, and whether it comes
  from your config or from team rules.
- **Usage** (folded): each profile's part of the last 7 days. Click a profile
  to see its heaviest projects, each checked against today's rules, and the
  conversations that send the most tokens with each answer. Folded, the line
  says how many projects deserve a look. It is the same estimate as
  [`cprofile usage`](commands.md#usage), and it changes nothing.
- **Conversations kept** (folded): how long each profile keeps its
  conversations.
- **Storage** (folded): the disk space of each profile.
- **Terminal** (folded): the app that "Open…" starts Claude Code in.

## What you can do

| Action | How |
|---|---|
| Open Claude Code in a profile | Click the profile. You pick the project folder, then a terminal window opens there |
| Create a profile | "New profile…", or ⌘N |
| Set the default profile | Hover the profile, then "Set as default" |
| Create a rule | "New rule…" under the rules. You pick a folder, then where the rule applies (every repository of its owner, this repository, or the folder), the profile, and whether to lock it |
| Remove a rule | Hover one of your own rules, then the bin icon and "Remove rule". Team rules cannot be removed |
| Remove a profile | Hover the profile, then the bin icon. The row says what goes and what stays before you confirm |
| Change how long conversations are kept | Unfold "Conversations kept": 30 days, 90 days, 1 year, or a number of days |
| Move big conversations to the Trash | Unfold "Storage", click a profile, then the bin icon of a conversation and "Move to Trash" |
| Choose the terminal app | Unfold "Terminal" and click one. "System default" is the app macOS opens `.command` files with, usually Terminal |
| Open the config file | ⌘, |
| Quit | ⌘Q |

Opening Claude Code from the app goes through the shim, so a locked rule on
the folder you pick still has the last word.

Removing a profile works as `cprofile remove` does: the profile's Claude Code
folder is kept unless you tick "Also delete its folder and sign out", which is
`cprofile remove --purge`.

## What it does not do yet

- A rule that needs both a folder and a git remote, or a pattern typed by
  hand, is created with `cprofile rule add` or in `config.toml`.
- The terminal list holds the installed apps that declare they run script
  files (Terminal, and others such as Ghostty, WezTerm or kitty). Opening
  Claude Code was tested with Terminal only. An app that does not accept a
  script file, such as Alacritty, is not listed.
- Claude Desktop is not handled.
- The app is in English only.
- It is not signed with an Apple developer certificate. See
  [installation.md](installation.md#why-curl-and-not-the-browser).
