# Contributing

Bug reports, ideas and pull requests are welcome.

## Reporting a problem

Run `orberus doctor` first, then [open an issue](https://github.com/Tz-dev0/orberus/issues)
with its output, the output of `orberus status`, and the versions of
`orberus`, Claude Code and macOS. Remove account addresses and private paths.

For a security problem, see [SECURITY.md](SECURITY.md) instead.

## Building

You need [Rust](https://rustup.rs). The menu bar app also needs Node.js.

```sh
cargo build                 # the CLI
cargo test                  # unit tests, and end-to-end tests with a fake claude

cd app
npm install
npm run dev                 # the app's interface alone, in a browser, with sample data
npm run tauri dev           # the real app
```

To try your build without touching your own setup, point it at a throwaway
folder:

```sh
ORBERUS_HOME=/tmp/cp-test cargo run -- add work
```

`scripts/smoke.sh <path to orberus>` runs the everyday commands in an empty
folder and prints how each one ends.

## Layout

| Path | Content |
|---|---|
| `src/` | The core and the CLI. `resolve.rs` picks the profile, `launch.rs` finds and starts the real Claude Code, `main.rs` is the CLI and the shim |
| `tests/shim.rs` | End-to-end tests |
| `app/src/` | The app's interface, in TypeScript without a framework |
| `app/src-tauri/` | The app's Rust side (Tauri 2). It is a separate crate, so the CLI does not pull Tauri's dependencies |
| `scripts/` | Packaging and smoke test |
| `docs/` | The documentation |

## Before you open a pull request

```sh
cargo fmt
cargo clippy --all-targets
cargo test
```

Run the same three in `app/src-tauri` when you touch the app.

A few rules the project holds to:

- **The shim stays fast.** On the path that starts Claude Code: no other
  process, no network, no heavy dependency.
- **Fail clearly.** When the config is wrong, the shim stops with a message.
  It never starts Claude Code with an account picked at random.
- **Every resolution rule has a test.** Add a test before changing the order
  in which profiles are chosen.
- **Few dependencies.** Say why in the pull request when you add one.
- **Check before you claim.** Do not write that Claude Code behaves in some
  way unless you tried it or the official documentation says so.
- Code, comments and messages are in English.

The project is for people with several legitimate, paid accounts. Features
that rotate accounts to get around usage limits will not be merged.

## Releasing (maintainers)

Set the version in `Cargo.toml`, `app/src-tauri/Cargo.toml`,
`app/src-tauri/tauri.conf.json` and `app/package.json`, run `cargo build` in
both crates so the lock files follow, commit, then push a tag with the same
number:

```sh
git tag v0.1.2
git push origin v0.1.2
```

The `Release` workflow runs the tests, builds a universal CLI with
`scripts/package.sh` and a universal app with `scripts/package-app.sh`, and
attaches both archives to a new GitHub release.
