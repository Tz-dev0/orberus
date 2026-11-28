# Claude Desktop

Claude Desktop holds one account at a time. Its account menu switches
between a personal account and a Team or Enterprise account of the same
e-mail address; with two addresses, Anthropic's help page says to log out
and log in again.

Orberus switches it for you. Each profile keeps its own sign-in, and a
switch closes Claude Desktop, then opens it again on the profile you name:

```sh
orberus desktop work
```

A switch closes the app and opens it again. From outside, the account of a
Claude Desktop that is open cannot be changed: that would take modifying the
app or handling its session cookies, and Orberus does neither.

This is not a feature of Claude Desktop. It relies on how the app is built
and on what was tried with one version of it: read
[what was tried](#what-was-tried) before you count on it.

## Using it

From the menu bar app, unfold "Claude Desktop" and click "Switch" next to a
profile. The line says what closes, and a second click does it. From a
terminal:

```text
$ orberus desktop
┌─────────┬────────────────┬─────────────────────────────────────────────────────────────┐
│ Profile │ Claude Desktop │ Its data                                                    │
├─────────┼────────────────┼─────────────────────────────────────────────────────────────┤
│ client  │ not opened yet │ ~/.orberus/desktop/client                                   │
│ perso   │ open           │ ~/Library/Application Support/Claude (Claude Desktop's own) │
│ work    │ closed         │ ~/.orberus/desktop/work                                     │
└─────────┴────────────────┴─────────────────────────────────────────────────────────────┘

Switch with: orberus desktop <profile>
  One Claude Desktop at a time: a switch closes it, then opens it again on
  that profile's account. Each profile keeps its own sign-in.

$ orberus desktop work
Claude Desktop is open on 'perso'. It closes, and what runs in it stops:
a conversation that is answering, a task.
Switch Claude Desktop to 'work'? [y/N] y
✓ Claude Desktop switched to 'work' (it was on 'perso')

$ orberus desktop work
✓ Claude Desktop was already on 'work': brought to the front
```

The first time Claude Desktop opens on a profile, it is empty: sign in there
with the account of that profile.

When Claude Desktop is closed, there is nothing to switch from: the command
opens it on the profile without a question, and the app's button says
"Open".

## What a switch closes

Every Claude Desktop that is open on another profile, with what runs in it:
a conversation that is answering, a task, a session of its Code tab. Nothing
is deleted. Each profile's data stays where it is, and Claude Desktop finds
it again when you switch back.

- **You are asked first.** In a terminal, the command says what closes and
  waits for a yes. Outside a terminal it needs `--yes`. In the menu bar app,
  it takes a second click.
- **Claude Desktop is asked to quit, never forced.** macOS asks it the way
  its Quit menu does. If Claude Desktop has a question of its own before it
  quits, answer it. After ten seconds, Orberus says that it did not quit,
  opens nothing, and leaves it to you.
- **One at a time.** Claude Desktop opens on the profile once the one that
  was open is gone, the way you would quit it and open it again. Two never
  run together.
- **Except from inside Claude Desktop.** Typed in a terminal of the Claude
  Desktop that has to close, or run by a Claude Code session of its Code
  tab, the command would be closed with the app before it opened anything.
  There, the new one opens first: for a moment two are open, with two
  Claude icons in the Dock, then one.

## Which profile gets which data

- **The profile that reuses `~/.claude`** (created with `--native`) keeps
  Claude Desktop as it is today: its data folder, its account, its
  conversations. Nothing is moved or copied. Switching to it is the same as
  opening Claude Desktop from the Dock.
- **Every other profile** gets a data folder of its own,
  `~/.orberus/desktop/<profile>`, created the first time you switch to it.

Opened from the Dock, Claude Desktop always uses its own folder, so the
account of the profile that reuses `~/.claude`. To be on another profile's
account, switch from Orberus.

Without a profile that reuses `~/.claude`, the Claude Desktop you open from
the Dock is on no profile. A switch closes it like any other; open Claude
Desktop from the Dock to come back to it.

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

after it asked macOS to quit the Claude Desktop that was open. Four things
in there are Orberus' own care:

- **It looks before it starts.** Claude Desktop does not stop a second copy
  from opening on the same data folder, and two copies writing to one folder
  is how data gets damaged. Orberus lists the ones that run, and brings the
  profile's one to the front when it is there.
- **Old first, new after.** Claude Desktop does not mind running twice on
  two data folders, each with its icon in the Dock. So the one that is open
  is asked to quit, and the new one is started once it is gone. Orberus
  checks whether it was itself started by the Claude Desktop that has to
  close; if so, closing first would end the command before it opened
  anything, and the order is turned around.
- **A clean environment.** `open` hands the new app every variable of the
  program that ran it. Started from a terminal inside Claude Code, Claude
  Desktop would inherit that session's variables. Orberus starts it with the
  few a normally opened app has (`HOME`, `USER`, `PATH`…) and nothing else.
- **The Code tab follows the profile.** Claude Desktop reads
  `CLAUDE_CONFIG_DIR` to find Claude Code's folder, so on a profile it is
  given that profile's Claude Code folder. The usual Claude Desktop is given
  nothing and keeps `~/.claude`.

Claude Desktop is found by its identifier in an Applications folder, not by
its name, and it is never modified.

## Good to know

- **Each profile's Claude Desktop is a full one**: its own settings,
  connectors and extensions. A feature that downloads something large into
  the data folder, such as the virtual machine behind Cowork, downloads it
  once per profile.
- **Limits**: the 5-hour and weekly numbers that
  [`orberus limits`](commands.md#limits) shows are read from every data
  folder, so each account's numbers come from its own Claude Desktop.
- **Removing a profile keeps its Claude Desktop data**, as it keeps its
  Claude Code folder. `orberus remove --purge` deletes both, and refuses
  while Claude Desktop is open on that profile. Claude Desktop's own folder
  is never deleted.
- **Orberus never reads what is in these folders** apart from the plan usage
  file: not the sign-in, not the conversations.

## What was tried

With Claude Desktop 7.37095.0 on macOS 27, and a data folder that was never
signed in to:

- Claude Desktop opens on a data folder of its own, next to the usual one,
  and leaves the usual one's data alone;
- it receives `CLAUDE_CONFIG_DIR` and nothing of the environment it was
  started from;
- asked to quit by macOS, it was gone within a second, with no permission
  asked;
- with two open, a switch to the usual Claude Desktop brought it to the
  front and closed the other one, in under a second;
- without a yes, nothing closes;
- `remove --purge` is refused while Claude Desktop is open on the profile,
  and deletes its data once it is closed.

With the same version, on a Mac where Claude Desktop is in daily use: signed
in, with a Claude Code session open in its Code tab.

- A switch to a profile that was never signed in to, and back a few seconds
  later, twice. Each time Claude Desktop opened on the profile asked for and
  the one that was open closed. On the way back, the other one was gone a
  second after the daily one started. These switches were made when the new
  one still opened first, whatever the switch was asked from.
- The daily Claude Desktop came back signed in, with its conversations.
- The second time, the session of the Code tab was in the middle of a turn.
  The switch stopped that turn, and the conversation went on from there once
  Claude Desktop was back.
- During a switch, the Dock showed two Claude icons. This is why a switch
  now closes first.

Not tried yet:

- **a switch in the order it now follows**, closing first: asking Claude
  Desktop to quit, waiting for it and starting it were each tried, not yet
  in that order, and not with the Dock in sight;
- **signing in on a second data folder**, and so everything that depends on
  it: that the sign-in is still there after a switch away and back, and the
  Code tab of a signed-in profile (that it reads and writes in the profile's
  Claude Code folder, and where it keeps that login);
- a Claude Desktop with a conversation that is answering or a task under
  way when it is asked to quit: whether it asks first, and how long it
  takes. With a session of the Code tab in the middle of a turn, it was gone
  within eight seconds;
- the command typed in a terminal inside the Claude Desktop that closes;
- the Dock: whether a second Claude icon stays once the switch is over, and
  what a click on it does while Claude Desktop is on another profile;
- an update of Claude Desktop while it runs on a profile's folder;
- what macOS reopens after a restart: it may bring Claude Desktop back on
  its own folder.

## Not handled

- **Changing the account without closing the app.** Only Claude Desktop
  itself could do that.
- **Two accounts open side by side.** Orberus keeps one Claude Desktop open.
- **Choosing the account by folder.** Rules pick the account of `claude`;
  Claude Desktop is switched to a profile you name.
- **Disk space**: [`orberus disk`](commands.md#disk) counts the Claude Code
  folders only.
- **Windows and Linux.**
