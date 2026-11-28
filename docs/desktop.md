# Claude Desktop

Claude Desktop holds one account at a time. Its account menu switches
between a personal account and a Team or Enterprise account of the same
e-mail address; with two addresses, Anthropic's help page says to log out
and log in again.

Orberus gives each profile its own Claude Desktop instead: the same app,
opened once more on a data folder of its own, so each one stays signed in to
its account.

```sh
orberus desktop work
```

This is not a feature of Claude Desktop. It relies on how the app is built
and on what was tried with one version of it: read
[what was tried](#what-was-tried) before you count on it.

## Using it

From the menu bar app, unfold "Claude Desktop" and click "Open" next to a
profile. From a terminal:

```text
$ orberus desktop
┌─────────┬────────────────┬─────────────────────────────────────────────────────────────┐
│ Profile │ Claude Desktop │ Its data                                                    │
├─────────┼────────────────┼─────────────────────────────────────────────────────────────┤
│ client  │ not opened yet │ ~/.orberus/desktop/client                                   │
│ perso   │ closed         │ ~/Library/Application Support/Claude (Claude Desktop's own) │
│ work    │ open           │ ~/.orberus/desktop/work                                     │
└─────────┴────────────────┴─────────────────────────────────────────────────────────────┘

Open one with: orberus desktop <profile>

$ orberus desktop work
✓ Claude Desktop started for 'work'
  First time: sign in there with the account of this profile.
  If you sign in through your browser, quit the other Claude Desktop windows
  first: with two open, which one the browser comes back to was not checked.

$ orberus desktop work
✓ Claude Desktop of 'work' was already open: brought to the front
```

The first time, the new Claude Desktop is empty: sign in there with the
account of that profile. It stays signed in afterwards, like the usual one.

Opening a profile whose Claude Desktop already runs does not start another
one: it brings that one to the front.

## Which profile gets which Claude Desktop

- **The profile that reuses `~/.claude`** (created with `--native`) keeps
  Claude Desktop as it is today: its data folder, its account, its
  conversations. Nothing is moved or copied. Opening it from Orberus is the
  same as opening Claude Desktop from the Dock.
- **Every other profile** gets a data folder of its own,
  `~/.orberus/desktop/<profile>`, created the first time you open it.

Without a profile that reuses `~/.claude`, Claude Desktop opened from the
Dock belongs to no profile, and Orberus leaves it alone.

## How it works

Claude Desktop is built on Electron, and an Electron app keeps everything
that makes it "you" (the sign-in, the settings, the conversations it lists)
in one data folder. Orberus starts the app with that folder set to the
profile's:

```sh
open -n -a /Applications/Claude.app \
  --env CLAUDE_CONFIG_DIR=/Users/you/.orberus/profiles/work \
  --args --user-data-dir=/Users/you/.orberus/desktop/work
```

Three things in there are Orberus' own care:

- **It looks before it starts.** Claude Desktop does not stop a second copy
  from opening on the same data folder, and two copies writing to one folder
  is how data gets damaged. Orberus lists the copies that run, and brings the
  profile's one to the front when it is there.
- **A clean environment.** `open` hands the new app every variable of the
  program that ran it. Started from a terminal inside Claude Code, the new
  Claude Desktop would inherit that session's variables. Orberus starts it
  with the few a normally opened app has (`HOME`, `USER`, `PATH`…) and
  nothing else.
- **The Code tab follows the profile.** Claude Desktop reads
  `CLAUDE_CONFIG_DIR` to find Claude Code's folder, so the copy of a profile
  is given that profile's Claude Code folder. The usual Claude Desktop is
  given nothing and keeps `~/.claude`.

Claude Desktop is found by its identifier in an Applications folder, not by
its name, and it is never modified.

## Good to know

- **They are all named "Claude" in the Dock**, with the same icon. Use
  `orberus desktop <profile>` or the menu bar app to reach the right one:
  both bring the copy of that profile to the front.
- **Sign in with the other copies closed.** Signing in through the browser
  ends with the browser handing over to "Claude", and macOS knows one app by
  that name. With two copies open, which one receives it was not checked.
- **Each copy is a full Claude Desktop**: it uses its own memory, and keeps
  its own settings, connectors and extensions. A feature that downloads
  something large into the data folder, such as the virtual machine behind
  Cowork, downloads it once per copy.
- **Limits**: the 5-hour and weekly numbers that
  [`orberus limits`](commands.md#limits) shows are read from every copy, so
  each account's numbers come from its own Claude Desktop.
- **Removing a profile keeps its Claude Desktop data**, as it keeps its
  Claude Code folder. `orberus remove --purge` deletes both, and refuses
  while that Claude Desktop is open. Claude Desktop's own folder is never
  deleted.
- **Orberus never reads what is in these folders** apart from the plan usage
  file: not the sign-in, not the conversations.

## What was tried

With Claude Desktop 7.37095.0 on macOS 27:

- a second copy opens next to the first one on a data folder of its own, and
  leaves the first one's data alone;
- the copy of a profile receives `CLAUDE_CONFIG_DIR` and nothing of the
  environment it was started from;
- opening a profile that is already open brings its window to the front, the
  usual Claude Desktop included;
- `remove --purge` is refused while the profile's copy runs, and deletes its
  data once it is closed.

Not tried yet:

- **signing in to a second copy**, and so everything that depends on it:
  where the browser comes back to with two copies open, and the Code tab of
  a signed-in copy (that it reads and writes in the profile's Claude Code
  folder, and where it keeps that login);
- an update of Claude Desktop while several copies run;
- what macOS reopens after a restart: a copy it brings back by itself may
  come back as the usual Claude Desktop.

## Not handled

- **Choosing the account by folder.** Rules pick the account of `claude`;
  Claude Desktop is opened for a profile you name.
- **Telling the copies apart in the Dock** or in the app switcher.
- **Disk space**: [`orberus disk`](commands.md#disk) counts the Claude Code
  folders only.
- **Windows and Linux.**
