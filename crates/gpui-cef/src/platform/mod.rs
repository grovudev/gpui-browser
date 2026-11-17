//! Backend selection. `init` / `Runtime` / `Webview` are swapped per platform.

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
#[path = "cef/mod.rs"]
mod imp;

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
#[path = "unsupported.rs"]
mod imp;

pub use imp::{Runtime, Webview, init};
