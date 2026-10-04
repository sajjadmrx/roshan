# AGENTS.md

Guidance for anyone, human or coding agent, working on Roshan. Read this
before changing code. User-facing docs are in [README.md](README.md),
build details in [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md), and
contribution rules in [CONTRIBUTING.md](CONTRIBUTING.md).

## What Roshan is

A small, offline desktop utility (Rust + GPUI). Users save **sessions**
("Morning", "Deep work"…): ordered lists of real apps, shell commands and
links. Pressing **Run** launches them one by one, with an optional pause after
each item. Roshan opens at sign-in (can be turned off) and waits idle, so the
computer boots light and the user's setup starts when they decide.

It is **not** a startup manager, process manager, optimizer, automation
platform or tracker. Reject features that push it in those directions.

## Non-negotiable rules

- **Real data only.** Never add demo entries, a built-in app catalog,
  hardcoded app names, or bundled third-party icons. App names and icons come
  from the OS at runtime. Test fixtures stay inside `#[cfg(test)]`.
- **Offline and private.** No network requests, accounts, telemetry or
  analytics.
- **Honest status.** Report only what the OS told us: `Launched`, `Failed`
  (with the OS reason) or `Skipped`. Never claim an app is "running" or
  "ready".
- **Light.** No background services, no filesystem scanning while idle.
  Discovery runs only when the app picker opens; icons load lazily and are
  cached on disk.
- **Never write to a broken config.** If `roshan.toml` cannot be parsed, it is
  moved aside, never overwritten.
- **Commits.** Use only the repository owner's git identity. Do not add
  AI co-author trailers or "generated with" lines to commits or PRs.

## Layout

```text
crates/
  roshan-core/        Model, TOML config, launch engine. No OS or UI code.
    model.rs          Session, LaunchItem, ItemKind, AppTarget
    config.rs         load/save (atomic), schema, normalization
    engine.rs         sequential runner, RunEvent, CancelToken, Launcher trait
  roshan-platform/    OS integration behind one facade (lib.rs).
    windows.rs        AppsFolder discovery, IShellItemImageFactory icons,
                      ShellExecuteEx launch, registry start-at-login,
                      single-instance mutex
    macos.rs          .app scan, NSWorkspace icons, `open`, LaunchAgent
    linux.rs          XDG .desktop discovery, icon themes, Exec launch,
                      terminal detection, XDG autostart
    desktop_entry.rs  pure .desktop / Exec= parsing (tested on all OSes)
    unix.rs           shared macOS/Linux helpers
    icon_cache.rs     on-disk PNG/SVG icon cache
    open_target.rs    URL/path classification, allowed URL schemes
  roshan/             The GPUI app.
    main.rs           args (--run, --startup, --version), window setup
    app.rs            all state and behavior (Roshan struct)
    view.rs           root view, title bar, overlays host
    screens/          welcome, sessions list, session, settings
    overlays/         add picker, item editor, dialogs, sheet helpers
    ui.rs             shared widgets (buttons, segmented, tiles, logo)
    theme.rs          palette (light/dark) and component theme sync
    i18n.rs           languages, string lookup, RTL helpers
    locales/          en.toml, fa.toml
    assets/           icons (Lucide subset), fonts (Vazirmatn), logo
    build.rs          logo sizes + Windows icon/version resource
vendor/gpui-pre-windows/  Patched GPUI Windows backend (see ROSHAN_PATCH.md)
docs/                 DEVELOPMENT.md, media/ (posters and their HTML sources)
packaging/linux/      .desktop file
```

Keep the dependency direction: `roshan` → `roshan-platform` → `roshan-core`.
OS code belongs only in `roshan-platform`, behind `#[cfg]` modules with the
same function set per OS.

## Commands

```sh
cargo build                     # debug build
cargo run                       # run the app
cargo test                      # all unit tests (Windows: 30+)
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo run -p roshan-platform --example list_apps [filter]   # see discovery
```

Before finishing any change: `fmt`, `clippy -D warnings` and `test` must pass.
The macOS and Linux platform code can be type-checked from Windows:

```sh
rustup target add aarch64-apple-darwin x86_64-unknown-linux-gnu
cargo clippy -p roshan-core -p roshan-platform --target aarch64-apple-darwin --all-targets -- -D warnings
cargo clippy -p roshan-core -p roshan-platform --target x86_64-unknown-linux-gnu --all-targets -- -D warnings
```

The full GPUI app cannot be cross-compiled; CI builds it on real runners.

## Testing the UI safely

- Set `ROSHAN_CONFIG=<path>` to use a throwaway sessions file. Never test
  against the user's real `roshan.toml`.
- Only one instance runs at a time (named mutex on Windows); close Roshan
  before starting another build.
- Finishing the welcome screen enables start-at-login
  (`HKCU\...\CurrentVersion\Run`, value `Roshan`). Remove that entry after
  testing with a debug build.
- Running a session really launches apps. Prefer harmless targets
  (Calculator, Notepad, `echo`) and close what you opened.

## Text and translations

- Every user-facing string lives in `crates/roshan/locales/*.toml`; add new
  keys to **every** language. Tests check missing keys and lost
  `{placeholders}`.
- Persian follows [TYPOGRAPHY.md](TYPOGRAPHY.md): casual tone («رو»),
  no tanween, no trailing periods, no `!`, real ZWNJ half-spaces, Persian
  digits, «،» instead of «·», «روتین» for session. A test enforces the
  mechanical rules.
- GPUI cannot wrap right-to-left paragraphs: break long Persian strings with
  `\n` (about 40 characters per line). `I18n::directional` wraps each line
  in an RTL embedding; use `I18n::join` to combine fragments.
- Build layouts with `ui::hrow` / `ui::vstack` so they mirror in RTL.

## Design rules

- Palette from the logo: charcoal tile, cream bowl, one amber light.
- Primary actions are solid (`p.primary`: charcoal on light, cream on dark).
- Amber (`p.accent`) only means "on": running item, active wait, enabled
  switch, focused field.
- No gradients, glows or decorative tints; depth from borders and surface
  steps. Lucide icons at 1.75 stroke.
- GPUI panics if `.hover()` is set twice on one element; helpers like
  `ui::button` already set it.

## Vendored GPUI patch

`vendor/gpui-pre-windows` is `gpui-pre-windows` 0.3.7 with marked
`ROSHAN PATCH` changes: right-to-left glyph placement in DirectWrite, and no
maximize (no `WS_MAXIMIZEBOX`, caption double-click and `SC_MAXIMIZE`
swallowed). Keep patches minimal and documented in `ROSHAN_PATCH.md`.

## Releases

Manual only: Actions → **Release** → enter a version. It bumps
`Cargo.toml`, tags, builds Windows/macOS/Linux and creates a **draft**
release for review.
