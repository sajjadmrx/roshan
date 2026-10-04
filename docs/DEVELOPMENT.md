# Developing Roshan

This page is for people who want to build Roshan, understand how it works, or
contribute. If you just want to use it, the [README](../README.md) is all you
need. To contribute, also read [CONTRIBUTING.md](../CONTRIBUTING.md).

## Install and build

Roshan is written in Rust with [GPUI](https://www.gpui.rs/) (through
[GPUI Kit](https://gpui-kit.com)).

```sh
cargo build --release
# The binary is target/release/roshan(.exe)
```

Requirements: a recent stable Rust (developed with 1.97).
On Linux, also install GPUI's system libraries, for example on Debian/Ubuntu:

```sh
sudo apt install libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev \
  libvulkan-dev libx11-xcb-dev libxcb1-dev libfontconfig-dev libfreetype-dev
```

## Using Roshan

1. Create a session and give it a name and an icon.
2. **Add** items:
   - **Apps:** search the apps installed on this computer. Press Enter to add
     the best match, or use "Browse for a program…" for anything not listed.
   - **Command:** any command line, for example `npm run dev`. It runs with
     your system shell (`cmd.exe` on Windows, your login shell on macOS and
     Linux) in a new terminal window, or silently in the background if you
     turn the terminal off.
   - **Link / File:** a web link (`github.com` works), a file or a folder,
     opened with its default app.
3. Click an item to rename it, edit it, change its wait time, or move it.
   Drag items to reorder them.
4. Press **Run**. You can skip a wait or stop the session at any time.

### Command line

```text
roshan                 Open Roshan
roshan --run <name>    Open Roshan and run the session called <name>
roshan --version       Print the version
```

`--run` works well in a desktop shortcut, e.g. `roshan --run Morning`.
Combined with *Settings → Close after a successful run*, the shortcut starts
your routine and gets out of the way. Only one Roshan runs at a time. If it is
already open, starting it again brings the existing window forward (and
`--run` is ignored in that case).

### Start at sign-in

Finishing the first-run screen adds Roshan to the programs that start when
you sign in. *Settings → Open when you sign in* turns it on or off:

| OS      | Where it lives                                                        |
| ------- | --------------------------------------------------------------------- |
| Windows | `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (value `Roshan`) |
| macOS   | `~/Library/LaunchAgents/app.roshan.Roshan.plist`                      |
| Linux   | `~/.config/autostart/roshan.desktop`                                  |

The entry starts `roshan --startup`, and Roshan keeps it pointing at the
current executable if you move or update the app.

### The sessions file

Everything is stored in one TOML file:

| OS      | Location                                             |
| ------- | ---------------------------------------------------- |
| Windows | `%APPDATA%\Roshan\roshan.toml`                       |
| macOS   | `~/Library/Application Support/Roshan/roshan.toml`   |
| Linux   | `~/.config/Roshan/roshan.toml`                       |

Set `ROSHAN_CONFIG` to use another file, for example for a portable setup.
*Settings → Sessions file* reveals it. The file is safe to edit while Roshan is
closed:

```toml
schema = 1

[settings]
language = "en"
theme = "system"          # system | light | dark
close_after_run = false

[[session]]
name = "Morning"
badge = "sunrise"

[[session.item]]
name = "My VPN"
kind = "app"
wait_after_secs = 5
target = { type = "windows_app", id = "Vendor.App_abc123!App" }

[[session.item]]
name = "dev server"
kind = "command"
line = "npm run dev"
cwd = 'C:\work\site'      # optional
terminal = true           # false = run in the background without a window

[[session.item]]
name = "Docs"
kind = "open"
target = "https://example.com"
```

If the file cannot be read, Roshan never overwrites it. It moves the file
aside (`roshan.toml.broken-<time>`), says so, and starts with an empty list.

## Platform support

| Capability              | Windows                                   | macOS                                  | Linux                                         |
| ----------------------- | ----------------------------------------- | -------------------------------------- | --------------------------------------------- |
| App discovery           | Shell *AppsFolder* (Start menu "All apps": classic and Store apps) | `.app` bundles in the standard folders | XDG `.desktop` entries (incl. Flatpak, Snap) |
| Real icons              | `IShellItemImageFactory`                  | `NSWorkspace`                          | Icon theme lookup (PNG, SVG)                  |
| Launch apps             | `ShellExecuteEx`, with real errors (incl. a cancelled admin prompt) | `open -a`                  | Parsed `Exec=` line, no shell                 |
| Commands in a terminal  | New console (Windows Terminal if it is your default) | Terminal.app                | `$TERMINAL`, `xdg-terminal-exec`, or a known terminal; configurable |
| Status                  | **Built and tested**                      | Implemented; compiles, not yet tested on a Mac | Implemented; compiles, not yet tested on Linux |

Discovery only runs when you open the app picker, and icons are extracted
lazily for visible rows and cached on disk. Roshan has no background service.
When idle it uses no CPU.

## Privacy and security

- Roshan collects nothing. Its only network access is the update check:
  one request to GitHub's latest-release API (at most daily, can be turned
  off) and, on Windows, downloading that release's `.exe` and
  `SHA256SUMS.txt`. The download is verified before it is installed.
- Nothing runs unless you press **Run** (or start Roshan with `--run`).
- Apps are launched directly, without a shell. Only items you created as a
  **Command** go through a shell, exactly as you typed them, and the UI says
  so.
- Links are limited to `http`, `https`, `mailto`, `ftp` and `file`. Other
  schemes can trigger arbitrary protocol handlers, so they are refused.
- Roshan never asks for administrator rights. An app that needs them shows its
  own prompt.
- The sessions file can contain commands, so treat a file someone sends you
  like a script: read it before using it. On macOS and Linux it is created
  readable only by you.

## Project layout

```text
crates/
  roshan-core/       model, TOML config, launch engine. No OS or UI code.
  roshan-platform/   discovery, icons, launching. One module per OS.
  roshan/            the GPUI app: screens, overlays, theme, translations
vendor/
  gpui-pre-windows/  GPUI's Windows backend with a right-to-left text fix
```

`vendor/gpui-pre-windows` is the published `gpui-pre-windows` 0.3.7 with two
small patches: Arabic-script text (Persian) is no longer drawn mirrored, and
the window cannot be maximized. See
`vendor/gpui-pre-windows/ROSHAN_PATCH.md`.

### Adding a language

1. Copy `crates/roshan/locales/en.toml` to `<code>.toml` and translate the
   values. Keep the `{placeholders}`.
2. Add an entry to `LANGUAGES` in `crates/roshan/src/i18n.rs` (native name,
   right-to-left or not, native digits if any).
3. Run `cargo test`. It checks that every key exists and that no placeholder
   was lost.

For right-to-left languages, put explicit `\n` breaks in long strings. The
text engine does not wrap right-to-left paragraphs correctly yet, so each
line should fit the compact window (about 40 characters).

## Design

The look comes from the logo: a charcoal tile, the cream bowl of «ن» (the
last letter of «روشن», which also reads as a horizon) and one small amber
light rising out of it.

- Primary actions are solid: charcoal on the light theme, cream on the dark
  theme. There are no gradients, glows or decorative tints.
- Amber means "on" and nothing else: a running item, an active wait, an
  enabled switch, the focused field.
- Depth comes from borders and surface steps rather than shadows. The UI
  icons are [Lucide](https://lucide.dev) at a 1.75 px stroke.

The logo is rendered in Blender from `crates/roshan/assets/logo/roshan-logo.blend`
(scene `RoshanLogo`). The build script turns `roshan.png` into the Windows
icon and the in-app sizes, so re-render the PNG after changing the model.

### Persian copy

All Persian text follows [`TYPOGRAPHY.md`](../TYPOGRAPHY.md): a casual, modern
tone («رو», «می‌تونی»), no tanween, no trailing periods or exclamation marks,
real half-spaces (ZWNJ) and Persian digits. Fragments are joined with «،»,
because a middle dot looks like the Persian zero (۰). `cargo test` checks
these rules on every string in `locales/fa.toml`.

## Development

```sh
cargo test                                   # all unit tests
cargo run -p roshan-platform --example list_apps [filter]   # what discovery sees
cargo clippy --all-targets
```

## Updates

`crates/roshan-platform/src/update.rs` asks
`https://api.github.com/repos/sajjadmrx/roshan/releases/latest` for the newest
published release (drafts and pre-releases are never offered), at most once a
day and only when **Settings → Check for updates automatically** is on.

- **Windows:** the release's `Roshan-Setup-<version>.exe` is downloaded to
  `%LOCALAPPDATA%\Roshan\updates` and checked against `SHA256SUMS.txt`.
  **Restart to update** runs it with
  `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CLOSEAPPLICATIONS /RELAUNCH=1`
  and quits. The installer replaces the files and, because of `/RELAUNCH=1`,
  reopens Roshan with `--updated`; the new copy waits for the old one to
  exit. Other silent installs (an administrator deploying Roshan) do not open
  the app.
- **macOS and Linux:** Roshan only says a new version is out and links to the
  release page.
- `HTTPS_PROXY` / `ALL_PROXY` are respected. `ROSHAN_UPDATE_URL` points the
  check at another server for testing.

The Release workflow publishes exactly the file names the updater looks for,
so keep the two in sync.

## Windows installer

`packaging/windows/roshan.iss` is an [Inno Setup 6](https://jrsoftware.org/isinfo.php)
script. It installs per user into `%LOCALAPPDATA%\Programs\Roshan` with no
admin prompt (so updates never need one), adds a Start menu shortcut and an
optional desktop shortcut, and registers an uninstaller. Uninstalling removes
the app, its caches and the "open at sign-in" entry, and asks before deleting
the user's sessions in `%APPDATA%\Roshan` (kept by default; silent uninstalls
always keep them). Keep `AppId` unchanged forever: it ties updates and the
uninstaller together.

Build it locally after a release build:

```powershell
winget install JRSoftware.InnoSetup --scope user   # once
cargo build --release
./packaging/windows/build-installer.ps1 -Version 1.2.3   # -> dist\Roshan-Setup-1.2.3.exe
```

The script takes the icon and wizard pictures that `crates/roshan/build.rs`
generates from the logo, so the repository holds no installer artwork.

## Releasing

Releases are made by hand with the **Release** workflow:

1. Open the repository's **Actions** tab, choose **Release**, then
   **Run workflow** on the `main` branch.
2. Enter the new version without the `v`, for example `0.2.0`.
3. The workflow checks the version, sets it in `Cargo.toml`, commits
   "Release v0.2.0", tags it `v0.2.0`, and builds Roshan for Windows (x64,
   as `Roshan-Setup-0.2.0.exe`), macOS (Apple silicon and Intel) and Linux
   (x64), running the tests where it can.
4. It then creates a **draft** release with the files, a `SHA256SUMS.txt`
   and generated notes. Review it on the Releases page and publish it.

## Licenses of bundled material

Roshan is MIT licensed (see [LICENSE](../LICENSE)). It bundles icons from
[Lucide](https://lucide.dev) (ISC, `crates/roshan/assets/icons/LICENSE-LUCIDE`)
and the [Vazirmatn](https://github.com/rastikerdar/vazirmatn) typeface (SIL OFL
1.1, `crates/roshan/assets/fonts/OFL.txt`). The vendored GPUI crate in
`vendor/gpui-pre-windows` is Apache-2.0.
