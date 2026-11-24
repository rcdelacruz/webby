//! Webby: a small WebKit browser. The platform modules below each wrap
//! their operating system's WebKit port behind `webby_core::Shell`.

#![cfg_attr(windows, windows_subsystem = "windows")]

use std::path::PathBuf;
use webby_core::{Browser, Store};

#[cfg_attr(target_os = "linux", path = "platform/linux.rs")]
#[cfg_attr(target_os = "macos", path = "platform/macos.rs")]
#[cfg_attr(windows, path = "platform/windows.rs")]
mod platform;

fn main() {
    // WEBBY_PROFILE overrides where history, session and site data live.
    let profile = std::env::var_os("WEBBY_PROFILE").map(PathBuf::from).or_else(|| dirs::data_dir().map(|d| d.join("webby")));
    let browser = Browser::new(Store::open(profile.clone()));
    platform::run(browser, profile);
}
