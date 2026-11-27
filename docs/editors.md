# Editors and IDEs

Claude Code runs inside an editor in one of two ways, and Orberus stands in
front of both. Which one your editor uses decides whether there is anything
to turn on.

| Editor | How it starts Claude Code | What you do |
|---|---|---|
| A terminal inside any editor | You type `claude` | Nothing: the shim is in your `PATH` |
| JetBrains IDEs (IntelliJ IDEA, PyCharm, Android Studio, WebStorm, PhpStorm, GoLand…) | The plugin runs the `claude` command in the IDE's terminal | Nothing, see [below](#jetbrains-ides) |
| VS Code, Cursor and the other editors built on VS Code | The extension starts its own copy of Claude Code | `orberus ide on` |

## VS Code, Cursor and the editors built on VS Code

The Claude Code extension of these editors carries its own copy of Claude
Code and starts it directly. It does not look in your `PATH`, so the shim
never runs: without the step below, the extension always uses the same
account, whatever folder is open.

```sh
orberus ide on
```

This writes one setting in the user settings of every such editor found on
your Mac (or of one only: `orberus ide on "VS Code"`):

```json
"claudeCode.claudeProcessWrapper": "/Users/you/.orberus/ide/claude-wrapper"
```

It is a setting of the extension, described in
[Claude Code's documentation](https://code.claude.com/docs/en/vs-code): a
program to start in place of the extension's copy, which is handed that copy.
Orberus' wrapper picks the profile for the folder the editor has open, by
[the same rules](rules.md#how-a-profile-is-chosen) as in a terminal, then
starts the extension's copy with it. Reload the editor's window afterwards.

```text
$ orberus ide
┌─────────┬───────────────────────┬─────────────────────────────────────────────────────────┐
│ Editor  │ Claude Code extension │ Settings                                                │
├─────────┼───────────────────────┼─────────────────────────────────────────────────────────┤
│ Cursor  │ through Orberus       │ ~/Library/Application Support/Cursor/User/settings.json │
│ VS Code │ through Orberus       │ ~/Library/Application Support/Code/User/settings.json   │
└─────────┴───────────────────────┴─────────────────────────────────────────────────────────┘
```

`orberus ide off` removes the setting again, and `orberus uninstall` does it
for you: an editor is never left pointing to a program that is gone.

Good to know:

- **Your settings file is edited in one place.** The setting is added as the
  first one of the file; comments, order and layout stay as they are, and
  `off` takes out that line only. A file Orberus does not understand is left
  alone, and you are shown the line to add by hand.
- **Another wrapper is never replaced.** If the setting already names a
  program, your company's for instance, Orberus says so and changes nothing.
- **The setting applies to this Mac only.** The extension declares it as a
  machine setting, which the editor reads from your user settings: not from
  a project, and not in a window opened on another machine (SSH, a
  container), which uses that machine's settings.
- **An editor is found by its folders**, not by its name, so an editor built
  on VS Code that Orberus has never heard of is listed too. An editor profile
  that has its own settings is listed on its own line.
- **A locked rule holds in the editor too.** When the config is wrong, or a
  locked rule forbids the profile that was asked for, Claude Code does not
  start, and Orberus prints the reason where the extension reads errors.
- **With this setting on, the extension itself behaves a little differently.**
  From its documentation: a resumed conversation does not come back in plan
  mode, and the "still open somewhere else" notice is skipped.
- **To force a profile in one editor**, set `ORBERUS_PROFILE` in the
  extension's `claudeCode.environmentVariables` setting. A locked rule still
  wins.

What was tried: the wrapper in front of the extension's own copy of Claude
Code (extension 2.1.325), started the way the extension starts it. Cursor and
the other editors built on VS Code install the same extension; they were not
tried one by one.

## JetBrains IDEs

The JetBrains plugin has no copy of Claude Code: it runs the `claude` command
in the IDE's terminal. That is the shim, as long as the IDE's terminal has it
first in its `PATH`, which `orberus doctor` checks ("apps and scripts get the
shim too").

To be sure, or if the IDE finds another `claude` first, give the plugin the
shim's full path: **Settings → Tools → Claude Code [Beta] → Claude command**,
set to `/Users/you/.orberus/bin/claude`.

## Checking which account an editor uses

In a conversation, ask Claude Code to run this:

```sh
echo "profile=$ORBERUS_SESSION"
```

It prints the name of the profile Orberus picked. An empty answer means
Claude Code was started without Orberus.

## Not handled

- **Zed's own Claude agent** signs in by itself, and no way to put Orberus in
  front of it was found yet. `claude` typed in a terminal inside Zed goes
  through the shim, as anywhere.
- **Windows and Linux.** The paths used to find editors there follow VS Code's
  documentation and were never tried.
