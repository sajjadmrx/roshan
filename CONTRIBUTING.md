# Contributing to Roshan

Thank you for wanting to help. Roshan is a small project, and every bug
report, idea, translation and fix makes a real difference.

This guide explains how to help and what to keep in mind. You do not need to
read it all before opening an issue. Just start, and we will figure out the
rest together.

## Ways to help

### Report a bug

Open an [issue](https://github.com/sajjadmrx/roshan/issues) and include:

- What you did, what you expected, and what happened instead.
- Your system, for example "Windows 11 23H2" or "Ubuntu 24.04, GNOME".
- The Roshan version (shown at the bottom of **Settings**).
- A screenshot, if the problem is something you can see.
- If an item failed to open, the reason Roshan showed next to it.

If your sessions file might be involved, you can attach it, but read it first.
It contains the names of your apps and any commands you added. Remove anything
you would rather keep private.

### Suggest an idea

Open an issue and describe the problem you want to solve, not only the
solution you have in mind. It helps us find the simplest way to do it.

Please read [What Roshan is, and is not](#what-roshan-is-and-is-not) first, so
your idea fits the direction of the project.

### Translate Roshan

Roshan is ready for more languages. Each language is one text file.

1. Copy `crates/roshan/locales/en.toml` to a new file named after your
   language code, for example `de.toml`.
2. Translate the values. Keep the parts in curly braces, like `{n}`, exactly
   as they are.
3. Add your language to the `LANGUAGES` list in `crates/roshan/src/i18n.rs`:
   its code, its name written in that language, whether it is written right
   to left, and its own digits if it has them.
4. Run `cargo test`. It checks that no text is missing and that every
   `{placeholder}` survived.

For right-to-left languages, break long lines yourself with `\n` (about 40
characters per line). The text engine cannot wrap right-to-left paragraphs on
its own yet.

**Persian text** must follow [TYPOGRAPHY.md](TYPOGRAPHY.md): a casual, modern
tone, no tanween, no trailing periods or exclamation marks, real half-spaces
and Persian digits. The tests check these rules automatically.

### Write code

Small, focused changes are the easiest to review. For anything bigger than a
bug fix, please open an issue first so we can agree on the approach before you
spend time on it.

## What Roshan is, and is not

Roshan does one thing: it lets people save a set of real things on their
computer and open them in order, with timing, in one click.

It is **not** a startup manager, a process manager, a system optimizer, an
automation platform or a productivity tracker. We say no to features that
would push it in those directions, even good ones, to keep it small and
focused.

A few rules every change has to respect:

- **Real data only.** Roshan never shows made-up apps, demo entries or
  bundled third-party icons. App names and icons always come from the user's
  own system. Test fixtures stay inside tests.
- **Offline and private.** No network requests, no accounts, no telemetry,
  no analytics.
- **Honest status.** The interface never claims something worked when it did
  not, and never claims an app is "ready" when Roshan cannot know that.
- **Light by design.** Nothing runs in the background, and nothing scans the
  disk while Roshan is idle.
- **Calm interface.** Follow the existing design: solid colors, amber only
  for things that are "on", no gradients or glows. See the design notes in
  [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md#design).

## Setting up

Everything you need to build and run Roshan is in
[docs/DEVELOPMENT.md](docs/DEVELOPMENT.md). In short, with Rust installed:

```sh
cargo run
```

Use the `ROSHAN_CONFIG` environment variable to try things with a separate
sessions file, so your own sessions stay untouched:

```sh
ROSHAN_CONFIG=./test-sessions.toml cargo run
```

## Before you open a pull request

Please make sure these pass:

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

Then check this list:

- [ ] The change does one thing, and the description says why it is needed.
- [ ] New behavior has tests where it can be tested without a real desktop.
- [ ] Any new user-facing text exists in every language file.
- [ ] If the interface changed, the description includes before and after
      screenshots, in English and in Persian (right to left).
- [ ] Platform-specific code stays in `crates/roshan-platform`, and the core
      crate still has no operating system or interface code.
- [ ] If you changed macOS or Linux code, say whether you could test it on a
      real machine. That is fine either way; we just need to know.

We review pull requests as soon as we can. If something needs to change, we
will explain why and help you get it over the line.

## Commit messages

Write a short summary line in the imperative mood ("Add Linux terminal
setting", not "Added"), then a blank line and a few lines on why, if it is not
obvious.

## Being kind

Be respectful and patient with everyone, especially people who are new to
open source or writing in a second language. Criticize ideas, not people.
Harassment of any kind is not welcome here.

## License

By contributing, you agree that your contribution is released under the
project's [MIT License](LICENSE).
