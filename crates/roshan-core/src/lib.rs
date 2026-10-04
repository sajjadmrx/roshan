//! Platform-independent core of Roshan: the session model, the on-disk
//! configuration format and the sequential launch engine.
//!
//! Nothing in this crate talks to the operating system directly. The engine
//! drives a [`engine::Launcher`], which the platform crate implements.

pub mod config;
pub mod engine;
pub mod model;

pub use config::{Config, ConfigError, Settings, ThemePreference};
pub use engine::{
    CancelToken, ItemStatus, LaunchError, Launched, Launcher, RunEvent, RunSummary, SkipReason,
};
pub use model::{AppTarget, ItemKind, LaunchItem, MAX_WAIT_SECS, Session};
