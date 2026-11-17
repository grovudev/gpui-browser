//! Embed a real browser engine as a [gpui] element.
//!
//! CEF provides off-screen rendering on all supported desktop platforms:
//!
//! | OS      | Engine                        | How it is composited |
//! |---------|-------------------------------|----------------------|
//! | macOS   | Chromium (CEF)                | Off-screen rendering, composited as an ordinary gpui element. |
//! | Windows | Chromium (CEF)                | Off-screen rendering, composited as an ordinary gpui element. |
//! | Linux   | Chromium (CEF)                | Off-screen rendering, composited as an ordinary gpui element. |
//!
//! The CEF binaries are downloaded by `cef-dll-sys` during the first build.
//! `cargo run -p xtask` assembles a runnable platform bundle with those files.
//!
//! # Usage
//!
//! Initialize CEF with [`init`] before creating GPUI's application, then call
//! [`Runtime::start`] inside its application closure.
//!
//! ```no_run
//! fn main() {
//!     // 1. At the top of main: load CEF into the process.
//!     let runtime = gpui_cef::init(gpui_cef::RuntimeOptions::default()).unwrap();
//!
//!     // Hand the closure a *clone*. Moving the value itself in would run
//!     // cef_shutdown() the moment the closure returns.
//!     let cef = runtime.clone();
//!     gpui::application().run(move |cx| {
//!         // 2. Initialize CEF inside the GPUI application closure.
//!         cef.start(cx).unwrap();
//!         // ... cx.open_window(..., |window, cx| cx.new(|cx| Webview::new(...)))
//!     });
//!
//!     // 3. The message loop is done, so shut CEF down.
//!     runtime.shutdown();
//! }
//! ```
//!
//! # When the page needs the application
//!
//! A windowless browser has nowhere to show a dialog, a file picker or a popup
//! of its own, so what the page asks for that needs one arrives as a
//! [`WebviewEvent`] — subscribe with `cx.subscribe(&webview, ...)` — and is
//! answered through the [`Webview`]: [`Webview::resolve_dialog`],
//! [`Webview::resolve_file_dialog`], [`Webview::resolve_download`].
//! [`WebviewOptions`] holds the policies for popups, navigation, dialogs and clipboard access.

// The usage example above is deliberately a full `fn main`, since the ordering
// around it is the whole point.
#![allow(clippy::needless_doctest_main)]

mod event;
mod ipc;
mod message;
mod options;
mod platform;

pub mod automation;

pub use automation::ViewportMode;
pub use event::{
    DialogId, DownloadId, DownloadState, FileDialogId, FileDialogMode, JsDialogKind,
    TerminationStatus, WebviewEvent, WebviewState,
};
pub use ipc::{
    AsyncScriptResult, ConsoleEntry, DomElementInfo, DomImageInfo, DomSnapshotPayload, IpcMessage,
    NetworkEntry, NetworkLogPayload, format_compact_snapshot, format_network_log,
    generate_ipc_token, origin_from_url, parse_ipc_message,
};
pub use options::{
    ClipboardAccessFilter, CookieSpec, DialogPolicy, NavigationFilter, NavigationRequest,
    PdfMargins, PdfOptions, PopupPolicy, RuntimeOptions, SameSite, WebviewOptions,
};
pub use platform::{Runtime, Webview, init};

/// What a console line starts with when it is a message for the application.
/// The page never needs to spell it out: [`post_message_script`] does.
pub const MESSAGE_PREFIX: &str = "gpui-cef:message:";

/// JavaScript that evaluates `expression`, turns it into a string, and posts
/// it to the application as a [`WebviewEvent::Message`]. Pass it to
/// [`Webview::eval`], or embed it in a script that does.
///
/// Messages travel through the page's console, which both backends can
/// observe without code in the page's own process.
pub fn post_message_script(expression: &str) -> String {
    format!("console.debug({MESSAGE_PREFIX:?} + String({expression}))")
}

/// The message a console line carries, if it is one.
pub fn message_payload(line: &str) -> Option<&str> {
    line.strip_prefix(MESSAGE_PREFIX)
}

/// Builds an asynchronous JavaScript evaluation wrapper. Its untrusted result
/// is posted as a [`WebviewEvent::Message`] using the `eval_result` envelope;
/// validate it with [`parse_ipc_message`] before accepting it.
pub fn eval_async_script(
    request_id: u64,
    token: &str,
    source: &str,
) -> std::result::Result<String, serde_json::Error> {
    let token = serde_json::to_string(token)?;
    let post = post_message_script(
        "JSON.stringify({kind:'eval_result', token, request_id:requestId, ok, result})",
    );
    Ok(format!(
        r#"(() => {{
  const token = {token};
  const requestId = {request_id};
  let settled = false;
  let timer;
  const finish = (ok, value) => {{
    if (settled) return;
    settled = true;
    clearTimeout(timer);
    let result;
    try {{
      result = typeof value === 'string' ? value : JSON.stringify(value);
      if (result === undefined) result = String(value);
    }} catch (error) {{
      result = String(error);
      ok = false;
    }}
    result = String(result).slice(0, 8000);
    {post}
  }};
  timer = setTimeout(() => finish(false, 'Script timed out after 5 seconds'), 5000);
  (async () => {{
    try {{
      const value = await (async () => {{
{source}
      }})();
      finish(true, value);
    }} catch (error) {{
      finish(false, error && error.message ? error.message : String(error));
    }}
  }})();
}})();"#
    ))
}

/// Something went wrong bringing up the browser engine.
#[derive(Debug)]
pub enum Error {
    /// The Chromium Embedded Framework could not be loaded from the platform bundle.
    LibraryLoad(String),
    /// `cef_initialize` failed.
    Initialize,
    /// The backend cannot do this on this platform.
    Unsupported(&'static str),
    /// The webview backend could not be created.
    Backend(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::LibraryLoad(path) => write!(
                f,
                "failed to load the Chromium Embedded Framework near {path}; \
                 build the platform bundle with `cargo run -p xtask`"
            ),
            Error::Initialize => write!(f, "cef_initialize() failed"),
            Error::Unsupported(what) => write!(f, "not supported on this platform: {what}"),
            Error::Backend(msg) => write!(f, "failed to create the webview backend: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

/// The result type used throughout this crate.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_posted_message_reads_back_from_its_console_line() {
        let script = post_message_script("document.title");
        assert_eq!(
            script,
            r#"console.debug("gpui-cef:message:" + String(document.title))"#
        );
        assert_eq!(
            message_payload(r#"gpui-cef:message:{"a":1}"#),
            Some(r#"{"a":1}"#)
        );
        assert_eq!(message_payload("an ordinary log line"), None);
    }

    #[test]
    fn async_eval_wraps_source_in_a_tokenized_result_message() {
        let script = eval_async_script(42, "session\"token", "return document.title;").unwrap();

        assert!(script.contains(r#"const token = "session\"token";"#));
        assert!(script.contains("const requestId = 42;"));
        assert!(script.contains("kind:'eval_result'"));
        assert!(script.contains("return document.title;"));
        assert!(script.contains("Script timed out after 5 seconds"));
    }
}
