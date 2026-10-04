// No console window for release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod assets;
mod i18n;
mod overlays;
mod screens;
mod theme;
mod ui;
mod view;

use gpui_kit::{AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, point, px, size};
use roshan_core::Config;

use crate::app::Roshan;

const USAGE: &str = "\
Roshan: launch your apps and commands together, in order.

Usage:
  roshan                 Open Roshan
  roshan --run <name>    Open Roshan and run the session called <name>
  roshan --version       Print the version

Set ROSHAN_CONFIG to use a different sessions file.";

fn main() {
    let mut run_on_start = None;
    let mut just_updated = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--run" | "-r" => run_on_start = args.next(),
            // Passed by the sign-in entry; Roshan simply opens.
            flag if flag == roshan_platform::STARTUP_FLAG => {}
            // Passed by the previous copy after it installed an update.
            flag if flag == roshan_platform::update::UPDATED_FLAG => just_updated = true,
            "--version" | "-V" => {
                println!("roshan {}", env!("CARGO_PKG_VERSION"));
                return;
            }
            _ => {
                println!("{USAGE}");
                return;
            }
        }
    }

    let claimed = if just_updated {
        // The previous copy is still closing; give it a moment.
        roshan_platform::claim_single_instance_patiently(std::time::Duration::from_secs(15))
    } else {
        roshan_platform::claim_single_instance(true)
    };
    if !claimed {
        // Roshan is already open; its window was brought to the front.
        return;
    }
    roshan_platform::update::clean_up();

    // Keep an enabled sign-in entry pointing at this copy of Roshan.
    roshan_platform::refresh_start_at_login();

    let config_path = roshan_platform::config_path();
    let (config, broken_backup) = match Config::load(&config_path) {
        Ok(config) => (config, None),
        Err(err) => {
            // Never overwrite a file we could not read: move it aside first.
            eprintln!("roshan: {err}");
            let backup = Config::back_up_broken(&config_path).ok();
            (Config::default(), backup)
        }
    };

    gpui_kit::application()
        .with_assets(assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            assets::load_fonts(cx);
            cx.set_global(i18n::I18n::new("en"));
            cx.set_global(theme::Palette::light());
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let bounds = Bounds::centered(None, size(px(380.), px(580.)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Roshan".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(12.), px(15.))),
                }),
                app_owns_titlebar_drag: true,
                window_min_size: Some(size(px(340.), px(440.))),
                app_id: Some("roshan".into()),
                ..Default::default()
            };
            gpui_kit::open_window(options, cx, move |window, cx| {
                cx.new(|cx| {
                    let mut roshan = Roshan::new(config, config_path, broken_backup, window, cx);
                    if let Some(name) = run_on_start {
                        roshan.run_by_name(&name, window, cx);
                    }
                    roshan
                })
            })
            .expect("failed to open the Roshan window");
            cx.activate(true);
        });
}
