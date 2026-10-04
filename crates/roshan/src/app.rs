//! Application state and behaviour. Rendering lives in `screens/` and
//! `overlays/`; this file owns the data and everything that changes it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::StreamExt;
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{
    App, AppContext, Context, Entity, FocusHandle, Focusable, Image, ImageFormat, SharedString,
    Subscription, Task, Window,
};
use roshan_core::engine::{self, ItemStatus};
use roshan_core::{
    AppTarget, CancelToken, Config, ItemKind, LaunchItem, MAX_WAIT_SECS, RunEvent, RunSummary,
    Session, ThemePreference,
};
use roshan_platform::{DiscoveredApp, IconFormat, SystemLauncher};

use crate::i18n::{self, I18n};
use crate::theme;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Welcome,
    Sessions,
    Session(usize),
    Settings,
}

pub enum Overlay {
    Picker(PickerTab),
    Editor { item: usize },
    ConfirmDelete,
    Badges,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickerTab {
    Apps,
    Command,
    Open,
}

pub enum AppsState {
    NotLoaded,
    Loading(#[allow(dead_code)] Task<()>),
    Loaded(Arc<Vec<DiscoveredApp>>),
}

pub enum IconSlot {
    Loading,
    Ready(Arc<Image>),
    Missing,
}

pub struct RunState {
    pub session: usize,
    pub statuses: Vec<ItemStatus>,
    pub waiting: Option<Waiting>,
    pub summary: Option<RunSummary>,
    cancel: CancelToken,
    _tasks: Vec<Task<()>>,
}

#[derive(Clone, Copy)]
pub struct Waiting {
    pub index: usize,
    pub until: Instant,
    pub total: Duration,
}

impl RunState {
    pub fn is_active(&self) -> bool {
        self.summary.is_none()
    }

    pub fn done_count(&self) -> usize {
        self.statuses
            .iter()
            .filter(|s| !matches!(s, ItemStatus::Pending | ItemStatus::Launching))
            .count()
    }
}

/// Text inputs, created once and reused.
pub struct Inputs {
    pub rename: Entity<InputState>,
    pub search: Entity<InputState>,
    pub cmd_line: Entity<InputState>,
    pub cmd_name: Entity<InputState>,
    pub cmd_cwd: Entity<InputState>,
    pub open_target: Entity<InputState>,
    pub open_name: Entity<InputState>,
    pub item_name: Entity<InputState>,
    pub item_line: Entity<InputState>,
    pub item_cwd: Entity<InputState>,
    pub item_target: Entity<InputState>,
}

pub struct Roshan {
    pub config: Config,
    pub config_path: PathBuf,
    pub screen: Screen,
    pub overlay: Option<Overlay>,
    pub run: Option<RunState>,
    pub apps: AppsState,
    pub icons: HashMap<String, IconSlot>,
    pub notice: Option<SharedString>,
    pub renaming: bool,
    pub cmd_terminal: bool,
    /// Mirrors the OS setting; see `roshan_platform::start_at_login`.
    pub start_at_login: bool,
    pub open_error: bool,
    pub inputs: Inputs,
    pub font_family: SharedString,
    pub focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl Focusable for Roshan {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

fn input(window: &mut Window, cx: &mut App) -> Entity<InputState> {
    cx.new(|cx| InputState::new(window, cx))
}

impl Roshan {
    pub fn new(
        config: Config,
        config_path: PathBuf,
        broken_backup: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let inputs = Inputs {
            rename: input(window, cx),
            search: input(window, cx),
            cmd_line: input(window, cx),
            cmd_name: input(window, cx),
            cmd_cwd: input(window, cx),
            open_target: input(window, cx),
            open_name: input(window, cx),
            item_name: input(window, cx),
            item_line: input(window, cx),
            item_cwd: input(window, cx),
            item_target: input(window, cx),
        };
        let mut subscriptions = vec![
            cx.subscribe_in(&inputs.rename, window, Self::on_rename_event),
            cx.subscribe_in(
                &inputs.search,
                window,
                |this, _, event, window, cx| match event {
                    InputEvent::Change => cx.notify(),
                    // Enter adds the best match, so keyboard-only adding is quick.
                    InputEvent::PressEnter { .. } => {
                        if let Some(app) = this.filtered_apps(cx).into_iter().next() {
                            this.add_app(app, window, cx);
                        }
                    }
                    _ => {}
                },
            ),
            cx.subscribe_in(&inputs.open_target, window, |this, _, event, _, cx| {
                if matches!(event, InputEvent::Change) {
                    this.open_error = false;
                    cx.notify();
                }
            }),
            cx.observe_window_appearance(window, |this, window, cx| {
                if this.config.settings.theme == ThemePreference::System {
                    this.apply_theme(window, cx);
                }
            }),
        ];
        for field in [&inputs.cmd_line, &inputs.cmd_name, &inputs.open_name] {
            subscriptions.push(
                cx.subscribe_in(field, window, |this, _, event, window, cx| {
                    if let InputEvent::PressEnter { .. } = event {
                        this.submit_picker_form(window, cx);
                    }
                }),
            );
        }
        for field in [
            &inputs.item_name,
            &inputs.item_line,
            &inputs.item_cwd,
            &inputs.item_target,
        ] {
            subscriptions.push(
                cx.subscribe_in(field, window, |this, _, event, window, cx| match event {
                    InputEvent::Blur => this.commit_editor_fields(cx),
                    InputEvent::PressEnter { .. } => this.close_editor(window, cx),
                    _ => {}
                }),
            );
        }

        let screen = if config.settings.language.is_none() {
            Screen::Welcome
        } else {
            Screen::Sessions
        };
        let mut this = Self {
            config,
            config_path,
            screen,
            overlay: None,
            run: None,
            apps: AppsState::NotLoaded,
            icons: HashMap::new(),
            notice: None,
            renaming: false,
            cmd_terminal: true,
            start_at_login: roshan_platform::start_at_login(),
            open_error: false,
            inputs,
            font_family: SharedString::default(),
            focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        };
        this.apply_language(window, cx);
        if let Some(backup) = broken_backup {
            let i = cx.global::<I18n>();
            this.notice = Some(
                format!(
                    "{}
{}",
                    i.t("config_error.title"),
                    i.tf(
                        "config_error.body",
                        &[("path", &backup.display().to_string())]
                    )
                )
                .into(),
            );
        }
        this
    }

    // ---------------------------------------------------------- persistence

    pub fn save(&mut self, cx: &mut Context<Self>) {
        if let Err(e) = self.config.save(&self.config_path) {
            self.notice = Some(e.to_string().into());
        }
        cx.notify();
    }

    // ------------------------------------------------------- look and feel

    pub fn language_code(&self) -> &'static str {
        i18n::language(
            self.config
                .settings
                .language
                .as_deref()
                .unwrap_or_else(|| i18n::guess_language()),
        )
        .code
    }

    pub fn set_language(&mut self, code: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.config.settings.language = Some(code.to_owned());
        self.apply_language(window, cx);
        self.save(cx);
    }

    pub fn apply_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let code = self.language_code();
        cx.set_global(I18n::new(code));
        self.font_family = pick_font(code, cx);
        self.apply_theme(window, cx);
        self.refresh_placeholders(window, cx);
    }

    pub fn set_theme(
        &mut self,
        pref: ThemePreference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.config.settings.theme = pref;
        self.apply_theme(window, cx);
        self.save(cx);
    }

    fn apply_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dark = theme::resolve_dark(self.config.settings.theme, window.appearance());
        theme::apply(dark, self.font_family.clone(), Some(window), cx);
        cx.notify();
    }

    fn refresh_placeholders(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let i = cx.global::<I18n>();
        let pairs = [
            (self.inputs.search.clone(), i.t("picker.search")),
            (
                self.inputs.cmd_line.clone(),
                i.t("command.line_placeholder"),
            ),
            (
                self.inputs.cmd_name.clone(),
                i.t("command.name_placeholder"),
            ),
            (self.inputs.cmd_cwd.clone(), i.t("command.cwd_placeholder")),
            (
                self.inputs.open_target.clone(),
                i.t("open.target_placeholder"),
            ),
            (self.inputs.open_name.clone(), i.t("open.name_placeholder")),
            (
                self.inputs.item_line.clone(),
                i.t("command.line_placeholder"),
            ),
            (self.inputs.item_cwd.clone(), i.t("command.cwd_placeholder")),
            (
                self.inputs.item_target.clone(),
                i.t("open.target_placeholder"),
            ),
        ];
        for (input, text) in pairs {
            input.update(cx, |state, cx| state.set_placeholder(text, window, cx));
        }
    }

    // ------------------------------------------------------------ navigation

    pub fn go(&mut self, screen: Screen, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_rename(window, cx);
        self.overlay = None;
        self.screen = screen;
        if let Screen::Session(ix) = screen {
            self.load_item_icons(ix, cx);
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    pub fn finish_welcome(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let code = self.language_code();
        self.set_language(code, window, cx);
        // Roshan starts with the computer by default; Settings can turn it off.
        self.set_start_at_login(true, cx);
        self.go(Screen::Sessions, window, cx);
    }

    pub fn set_start_at_login(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if let Err(err) = roshan_platform::set_start_at_login(enabled) {
            self.notice = Some(err.into());
        }
        self.start_at_login = roshan_platform::start_at_login();
        cx.notify();
    }

    pub fn current_session(&self) -> Option<usize> {
        match self.screen {
            Screen::Session(ix) if ix < self.config.sessions.len() => Some(ix),
            _ => None,
        }
    }

    // -------------------------------------------------------------- sessions

    pub fn create_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = i18n::t(cx, "sessions.default_name");
        let mut session = Session::new(name.to_string());
        session.badge = Some(
            crate::assets::BADGES[self.config.sessions.len() % crate::assets::BADGES.len()]
                .to_owned(),
        );
        self.config.sessions.push(session);
        self.save(cx);
        let ix = self.config.sessions.len() - 1;
        self.go(Screen::Session(ix), window, cx);
        self.start_rename(window, cx);
    }

    pub fn start_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.current_session() else {
            return;
        };
        let name = self.config.sessions[ix].name.clone();
        self.renaming = true;
        self.inputs.rename.update(cx, |state, cx| {
            state.set_value(name, window, cx);
            state.select_all(window, cx);
            state.focus(window, cx);
        });
        cx.notify();
    }

    fn on_rename_event(
        &mut self,
        _: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
            self.finish_rename(window, cx);
        }
    }

    pub fn finish_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.renaming {
            return;
        }
        self.renaming = false;
        let name = self.inputs.rename.read(cx).value().trim().to_owned();
        if let Some(ix) = self.current_session()
            && !name.is_empty()
            && self.config.sessions[ix].name != name
        {
            self.config.sessions[ix].name = name;
            self.save(cx);
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    pub fn set_badge(&mut self, badge: &str, cx: &mut Context<Self>) {
        if let Some(ix) = self.current_session() {
            self.config.sessions[ix].badge = Some(badge.to_owned());
            self.overlay = None;
            self.save(cx);
        }
    }

    pub fn delete_current_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.current_session() else {
            return;
        };
        if self.is_running(ix) {
            return;
        }
        self.config.sessions.remove(ix);
        // Keep a finished run pointing at the right session (or drop it).
        if let Some(run) = &mut self.run {
            if run.session == ix {
                self.run = None;
            } else if run.session > ix {
                run.session -= 1;
            }
        }
        self.save(cx);
        self.go(Screen::Sessions, window, cx);
    }

    // ----------------------------------------------------------------- items

    pub fn add_item(&mut self, item: LaunchItem, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.current_session() else {
            return;
        };
        self.clear_finished_run(ix);
        self.config.sessions[ix].items.push(item);
        self.save(cx);
        self.load_item_icons(ix, cx);
        self.overlay = None;
        window.focus(&self.focus, cx);
    }

    pub fn move_item(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        let Some(ix) = self.current_session() else {
            return;
        };
        if self.is_running(ix) {
            return;
        }
        self.clear_finished_run(ix);
        self.config.sessions[ix].move_item(from, to);
        if let Some(Overlay::Editor { item }) = &mut self.overlay
            && *item == from
        {
            *item = to;
        }
        self.save(cx);
    }

    pub fn remove_item(&mut self, item: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.current_session() else {
            return;
        };
        if self.is_running(ix) || item >= self.config.sessions[ix].items.len() {
            return;
        }
        self.clear_finished_run(ix);
        self.config.sessions[ix].items.remove(item);
        self.overlay = None;
        window.focus(&self.focus, cx);
        self.save(cx);
    }

    pub fn set_wait(&mut self, item: usize, secs: u32, cx: &mut Context<Self>) {
        if let Some(ix) = self.current_session()
            && let Some(it) = self.config.sessions[ix].items.get_mut(item)
        {
            it.wait_after_secs = secs.min(MAX_WAIT_SECS);
            self.save(cx);
        }
    }

    pub fn open_editor(&mut self, item: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(ix) = self.current_session() else {
            return;
        };
        if self.is_running(ix) {
            return;
        }
        let Some(it) = self.config.sessions[ix].items.get(item).cloned() else {
            return;
        };
        let set = |input: &Entity<InputState>, value: String, window: &mut Window, cx: &mut App| {
            input.update(cx, |state, cx| state.set_value(value, window, cx));
        };
        set(&self.inputs.item_name, it.name.clone(), window, cx);
        match &it.kind {
            ItemKind::Command { line, cwd, .. } => {
                set(&self.inputs.item_line, line.clone(), window, cx);
                let cwd = cwd
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                set(&self.inputs.item_cwd, cwd, window, cx);
            }
            ItemKind::Open { target } => set(&self.inputs.item_target, target.clone(), window, cx),
            ItemKind::App { .. } => {}
        }
        self.overlay = Some(Overlay::Editor { item });
        cx.notify();
    }

    /// Writes the editor's text fields back into the item.
    pub fn commit_editor_fields(&mut self, cx: &mut Context<Self>) {
        let (Some(ix), Some(Overlay::Editor { item })) = (self.current_session(), &self.overlay)
        else {
            return;
        };
        let item = *item;
        let value = |input: &Entity<InputState>, cx: &App| input.read(cx).value().trim().to_owned();
        let name = value(&self.inputs.item_name, cx);
        let line = value(&self.inputs.item_line, cx);
        let cwd = value(&self.inputs.item_cwd, cx);
        let target = value(&self.inputs.item_target, cx);
        let Some(it) = self.config.sessions[ix].items.get_mut(item) else {
            return;
        };
        let before = it.clone();
        if !name.is_empty() {
            it.name = name;
        }
        match &mut it.kind {
            ItemKind::Command {
                line: l, cwd: c, ..
            } => {
                if !line.is_empty() {
                    *l = line;
                }
                *c = (!cwd.is_empty()).then(|| PathBuf::from(cwd));
            }
            ItemKind::Open { target: t } if !target.is_empty() => *t = target,
            _ => {}
        }
        if *it != before {
            self.save(cx);
        }
    }

    pub fn set_item_terminal(&mut self, item: usize, terminal: bool, cx: &mut Context<Self>) {
        if let Some(ix) = self.current_session()
            && let Some(LaunchItem {
                kind: ItemKind::Command { terminal: t, .. },
                ..
            }) = self.config.sessions[ix].items.get_mut(item)
        {
            *t = terminal;
            self.save(cx);
        }
    }

    pub fn close_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_editor_fields(cx);
        self.overlay = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    // ---------------------------------------------------------------- picker

    pub fn open_picker(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for input in [
            &self.inputs.search,
            &self.inputs.cmd_line,
            &self.inputs.cmd_name,
            &self.inputs.cmd_cwd,
            &self.inputs.open_target,
            &self.inputs.open_name,
        ] {
            input.update(cx, |state, cx| state.set_value("", window, cx));
        }
        self.cmd_terminal = true;
        self.open_error = false;
        self.set_picker_tab(PickerTab::Apps, window, cx);
        self.load_apps(false, cx);
    }

    pub fn set_picker_tab(&mut self, tab: PickerTab, window: &mut Window, cx: &mut Context<Self>) {
        self.overlay = Some(Overlay::Picker(tab));
        let focus = match tab {
            PickerTab::Apps => &self.inputs.search,
            PickerTab::Command => &self.inputs.cmd_line,
            PickerTab::Open => &self.inputs.open_target,
        };
        focus.update(cx, |state, cx| state.focus(window, cx));
        cx.notify();
    }

    pub fn load_apps(&mut self, force: bool, cx: &mut Context<Self>) {
        if !force && !matches!(self.apps, AppsState::NotLoaded) {
            return;
        }
        let task = cx.spawn(async move |this, cx| {
            let apps = cx
                .background_executor()
                .spawn(async move { roshan_platform::discover_apps() })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.apps = AppsState::Loaded(Arc::new(apps));
                cx.notify();
            });
        });
        self.apps = AppsState::Loading(task);
        cx.notify();
    }

    /// Apps matching the search box, best matches first.
    pub fn filtered_apps(&self, cx: &App) -> Vec<DiscoveredApp> {
        let AppsState::Loaded(apps) = &self.apps else {
            return Vec::new();
        };
        let query = self.inputs.search.read(cx).value().trim().to_lowercase();
        if query.is_empty() {
            return apps.as_ref().clone();
        }
        let mut scored: Vec<(u8, &DiscoveredApp)> = apps
            .iter()
            .filter_map(|app| match_score(&app.name.to_lowercase(), &query).map(|s| (s, app)))
            .collect();
        scored.sort_by_key(|(score, _)| *score);
        scored.into_iter().map(|(_, app)| app.clone()).collect()
    }

    pub fn add_app(&mut self, app: DiscoveredApp, window: &mut Window, cx: &mut Context<Self>) {
        let item = LaunchItem::new(app.name, ItemKind::App { target: app.target });
        self.add_item(item, window, cx);
    }

    pub fn browse_program(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update_in(cx, |this, window, cx| {
                    let item = LaunchItem::new(
                        roshan_platform::name_for_path(&path),
                        ItemKind::App {
                            target: roshan_platform::target_for_path(&path),
                        },
                    );
                    this.add_item(item, window, cx);
                });
            }
        })
        .detach();
    }

    /// Picks a file or folder into `target`.
    pub fn browse_into(
        &mut self,
        target: Entity<InputState>,
        directories: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths = cx.prompt_for_paths(gpui_kit::PathPromptOptions {
            files: !directories,
            directories,
            multiple: false,
            prompt: None,
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await
                && let Some(path) = paths.into_iter().next()
            {
                let _ = this.update_in(cx, |this, window, cx| {
                    target.update(cx, |state, cx| {
                        state.set_value(path.display().to_string(), window, cx)
                    });
                    this.open_error = false;
                    this.commit_editor_fields(cx);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn submit_picker_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = |input: &Entity<InputState>, cx: &App| input.read(cx).value().trim().to_owned();
        match self.overlay {
            Some(Overlay::Picker(PickerTab::Command)) => {
                let line = value(&self.inputs.cmd_line, cx);
                if line.is_empty() {
                    return;
                }
                let name = value(&self.inputs.cmd_name, cx);
                let cwd = value(&self.inputs.cmd_cwd, cx);
                let name = if name.is_empty() {
                    command_title(&line)
                } else {
                    name
                };
                let item = LaunchItem::new(
                    name,
                    ItemKind::Command {
                        line,
                        cwd: (!cwd.is_empty()).then(|| PathBuf::from(cwd)),
                        terminal: self.cmd_terminal,
                    },
                );
                self.add_item(item, window, cx);
            }
            Some(Overlay::Picker(PickerTab::Open)) => {
                let target = value(&self.inputs.open_target, cx);
                let classified = roshan_platform::open_target::classify(&target);
                let Ok(open) = classified else {
                    self.open_error = true;
                    cx.notify();
                    return;
                };
                let name = value(&self.inputs.open_name, cx);
                let name = if name.is_empty() {
                    open_title(&open)
                } else {
                    name
                };
                self.add_item(LaunchItem::new(name, ItemKind::Open { target }), window, cx);
            }
            _ => {}
        }
    }

    // ----------------------------------------------------------------- icons

    pub fn icon(&mut self, target: &AppTarget, cx: &mut Context<Self>) -> Option<Arc<Image>> {
        let key = target.cache_key();
        match self.icons.get(&key) {
            Some(IconSlot::Ready(image)) => return Some(image.clone()),
            Some(_) => return None,
            None => {}
        }
        self.icons.insert(key.clone(), IconSlot::Loading);
        let target = target.clone();
        cx.spawn(async move |this, cx| {
            let icon = cx
                .background_executor()
                .spawn(async move { roshan_platform::app_icon(&target) })
                .await;
            let _ = this.update(cx, |this, cx| {
                let slot = match icon {
                    Some(icon) => {
                        let format = match icon.format {
                            IconFormat::Png => ImageFormat::Png,
                            IconFormat::Svg => ImageFormat::Svg,
                        };
                        IconSlot::Ready(Arc::new(Image::from_bytes(format, icon.bytes)))
                    }
                    None => IconSlot::Missing,
                };
                this.icons.insert(key, slot);
                cx.notify();
            });
        })
        .detach();
        None
    }

    fn load_item_icons(&mut self, ix: usize, cx: &mut Context<Self>) {
        let targets: Vec<AppTarget> = self.config.sessions[ix]
            .items
            .iter()
            .filter_map(|item| match &item.kind {
                ItemKind::App { target } => Some(target.clone()),
                _ => None,
            })
            .collect();
        for target in targets {
            self.icon(&target, cx);
        }
    }

    // ------------------------------------------------------------------- run

    pub fn is_running(&self, session: usize) -> bool {
        self.run
            .as_ref()
            .is_some_and(|r| r.session == session && r.is_active())
    }

    pub fn any_running(&self) -> bool {
        self.run.as_ref().is_some_and(RunState::is_active)
    }

    pub fn run_for(&self, session: usize) -> Option<&RunState> {
        self.run.as_ref().filter(|r| r.session == session)
    }

    fn clear_finished_run(&mut self, session: usize) {
        if self
            .run
            .as_ref()
            .is_some_and(|r| r.session == session && !r.is_active())
        {
            self.run = None;
        }
    }

    pub fn start_run(&mut self, session: usize, cx: &mut Context<Self>) {
        if self.any_running() || session >= self.config.sessions.len() {
            return;
        }
        let items = self.config.sessions[session].items.clone();
        if items.is_empty() {
            return;
        }
        let cancel = CancelToken::new();
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let launcher = SystemLauncher {
            terminal: self.config.settings.terminal.clone(),
        };
        let worker_cancel = cancel.clone();
        let count = items.len();
        std::thread::Builder::new()
            .name("roshan-run".into())
            .spawn(move || {
                engine::run(&items, &launcher, &worker_cancel, |event| {
                    let _ = tx.unbounded_send(event);
                });
            })
            .expect("failed to start the launch thread");

        let events = cx.spawn(async move |this, cx| {
            while let Some(event) = rx.next().await {
                if this
                    .update(cx, |this, cx| this.on_run_event(event, cx))
                    .is_err()
                {
                    break;
                }
            }
        });
        // Repaint the countdown while waiting between items.
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let keep_going = this
                    .update(cx, |this, cx| {
                        let active = this.any_running();
                        if this.run.as_ref().is_some_and(|r| r.waiting.is_some()) {
                            cx.notify();
                        }
                        active
                    })
                    .unwrap_or(false);
                if !keep_going {
                    break;
                }
            }
        });
        self.run = Some(RunState {
            session,
            statuses: vec![ItemStatus::Pending; count],
            waiting: None,
            summary: None,
            cancel,
            _tasks: vec![events, ticker],
        });
        cx.notify();
    }

    fn on_run_event(&mut self, event: RunEvent, cx: &mut Context<Self>) {
        let Some(run) = &mut self.run else {
            return;
        };
        match event {
            RunEvent::Item { index, status } => {
                if let Some(slot) = run.statuses.get_mut(index) {
                    *slot = status;
                }
            }
            RunEvent::Waiting { index, duration } => {
                run.waiting = Some(Waiting {
                    index,
                    until: Instant::now() + duration,
                    total: duration,
                });
            }
            RunEvent::WaitEnded { .. } => run.waiting = None,
            RunEvent::Finished(summary) => {
                run.waiting = None;
                run.summary = Some(summary);
                if summary.all_ok() && self.config.settings.close_after_run {
                    cx.spawn(async move |_, cx| {
                        cx.background_executor()
                            .timer(Duration::from_millis(1200))
                            .await;
                        cx.update(|cx| cx.quit());
                    })
                    .detach();
                }
            }
        }
        cx.notify();
    }

    /// `--run <name>`: opens the named session and starts it.
    pub fn run_by_name(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.screen == Screen::Welcome {
            return;
        }
        let wanted = name.trim().to_lowercase();
        let Some(ix) = self
            .config
            .sessions
            .iter()
            .position(|s| s.name.trim().to_lowercase() == wanted)
        else {
            return;
        };
        self.go(Screen::Session(ix), window, cx);
        self.start_run(ix, cx);
    }

    pub fn stop_run(&mut self, cx: &mut Context<Self>) {
        if let Some(run) = &self.run {
            run.cancel.cancel();
        }
        cx.notify();
    }

    pub fn skip_wait(&mut self, cx: &mut Context<Self>) {
        if let Some(run) = &self.run {
            run.cancel.skip_wait();
        }
        cx.notify();
    }

    pub fn dismiss_run(&mut self, cx: &mut Context<Self>) {
        if !self.any_running() {
            self.run = None;
        }
        cx.notify();
    }
}

/// Lower is better; `None` means no match.
fn match_score(name: &str, query: &str) -> Option<u8> {
    if name.starts_with(query) {
        Some(0)
    } else if name
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word.starts_with(query))
    {
        Some(1)
    } else if name.contains(query) {
        Some(2)
    } else {
        // Initials: "vsc" matches "Visual Studio Code".
        let initials: String = name
            .split(|c: char| !c.is_alphanumeric())
            .filter_map(|word| word.chars().next())
            .collect();
        initials.starts_with(query).then_some(3)
    }
}

/// A display name for a command line: the line itself when it is short,
/// otherwise its program name.
fn command_title(line: &str) -> String {
    if line.chars().count() <= 24 {
        return line.to_owned();
    }
    let first = line.split_whitespace().next().unwrap_or(line);
    let first = first.trim_matches(['"', '\'']);
    roshan_platform::name_for_path(std::path::Path::new(first))
}

fn open_title(target: &roshan_platform::OpenTarget) -> String {
    match target {
        roshan_platform::OpenTarget::Url(url) => url
            .split("://")
            .nth(1)
            .unwrap_or(url)
            .trim_start_matches("www.")
            .split(['/', '?', '#'])
            .next()
            .unwrap_or(url)
            .to_owned(),
        roshan_platform::OpenTarget::Path(path) => roshan_platform::name_for_path(path),
    }
}

/// Persian uses the bundled Vazirmatn; other languages use the system UI font.
fn pick_font(code: &str, cx: &App) -> SharedString {
    if code == "fa" {
        return crate::assets::PERSIAN_FONT.into();
    }
    let installed = cx.text_system().all_font_names();
    let has = |name: &str| installed.iter().any(|f| f == name);
    let candidates: &[&str] = &["Segoe UI Variable Text", "Segoe UI"];
    candidates
        .iter()
        .find(|name| has(name))
        .map_or(".SystemUIFont".into(), |name| SharedString::from(*name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranks_search_matches() {
        assert_eq!(match_score("telegram desktop", "tele"), Some(0));
        assert_eq!(match_score("visual studio code", "studio"), Some(1));
        assert_eq!(match_score("notepad++", "pad"), Some(2));
        assert_eq!(match_score("visual studio code", "vsc"), Some(3));
        assert_eq!(match_score("telegram", "xyz"), None);
        assert_eq!(
            match_score("cisco anyconnect secure mobility client", "telel"),
            None
        );
    }

    #[test]
    fn derives_titles() {
        assert_eq!(command_title("9router"), "9router");
        assert_eq!(command_title("npm run dev"), "npm run dev");
        assert_eq!(
            command_title("\"C:\\Tools\\srv.exe\" --port 8080 --verbose"),
            "srv"
        );
        assert_eq!(
            open_title(&roshan_platform::OpenTarget::Url(
                "https://www.github.com/x?y".into()
            )),
            "github.com"
        );
    }
}
