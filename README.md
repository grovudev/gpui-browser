# gpui-cef

[![CI](https://github.com/grovudev/gpui-cef/actions/workflows/ci.yml/badge.svg)](https://github.com/grovudev/gpui-cef/actions/workflows/ci.yml)

Embed a real browser engine into [gpui](https://github.com/zed-industries/zed/tree/main/crates/gpui).

| OS | Engine | How it is composited |
|---|---|---|
| macOS | **Chromium (CEF 156)** | Rendered off-screen and drawn as an ordinary gpui element |
| Windows | **Chromium (CEF 156)** | Rendered off-screen and drawn as an ordinary gpui element |
| Linux | **Chromium (CEF 156)** | Rendered off-screen and drawn as an ordinary gpui element |

CEF's matching binary distribution is downloaded automatically during the first
build by `cef-dll-sys`. `CEF_PATH` can optionally point to an existing CEF
distribution. Run `cargo run -p xtask` to assemble a runnable bundle in
`target/bundle` with the CEF runtime files included.

Launch the generated bundle with `open target/bundle/gpui-cef-browser.app` on
macOS, `target/bundle/run.sh` on Linux, or
`target/bundle/gpui-cef-browser.exe` on Windows.

Off-screen rendering turns the page into an ordinary gpui element, so it is
clipped and layered with the rest of the interface on all three platforms.

## Usage

```rust
use gpui_cef::{RuntimeOptions, Webview, WebviewOptions};

fn main() {
    // 1. At the top of main: load CEF into the process.
    let runtime = gpui_cef::init(RuntimeOptions::default()).unwrap();

    // Hand the closure a *clone*. Moving the value itself in would run
    // cef_shutdown() the moment the closure returns, taking the process with it.
    let cef = runtime.clone();
    gpui::application().run(move |cx| {
        // 2. Initialize CEF from inside gpui's application closure.
        cef.start(cx).unwrap();

        cx.open_window(Default::default(), |window, cx| {
            cx.new(|cx| Webview::new(window, cx, WebviewOptions::url("https://example.com")))
        })
        .unwrap();
    });

    // 3. The message loop is done, so shut CEF down.
    runtime.shutdown();
}
```

`Webview` is an ordinary gpui entity, so `div().child(webview.clone())` mixes it
with anything else.

### API

| API | CEF on macOS, Windows, and Linux |
|---|---|---|
| `load_url` / `reload` / `go_back` / `go_forward` / `stop` | yes |
| `eval` (run JavaScript), `url`, `title`, `is_loading` | yes |
| `can_go_back` / `can_go_forward` | yes |
| `WebviewEvent::Message` (page to app, via `post_message_script`) | yes |
| `Runtime::set_cookies` | yes |
| Popups: `WebviewOptions::popups`, `WebviewEvent::PopupRequested` | yes |
| Navigation filtering and `WebviewEvent::NavigationBlocked` | yes, for the page and its frames |
| JavaScript dialogs, file choosers, and downloads | yes |
| `WebviewEvent::DownloadUpdated` | progress, completion, cancellation, failure |
| `WebviewEvent::RenderProcessTerminated` / `LoadFailed` | yes |
| `print_to_pdf`, `WebviewEvent::PdfPrinted` | yes |

A page talks back to the application through its console: the script
`post_message_script(expr)` returns logs `expr` with a prefix, and the webview
emits it as `WebviewEvent::Message`. Anything in the page can do this, so treat
a message as input from the page.

`gpui` is provided by `gpui-kit`, allowing this crate to use the same GPUI
facade as applications built with gpui-component.

### When the page needs the application

A windowless browser has no window of its own to put a dialog, a file
picker, a popup or a "Save As" panel in, so whatever the page asks for that
needs one comes to the application as a `WebviewEvent`, and the application
answers through the webview:

```rust
use std::sync::Arc;
use gpui::PromptLevel;
use gpui_cef::{JsDialogKind, Webview, WebviewEvent, WebviewOptions};

let webview = cx.new(|cx| {
    Webview::new(window, cx, WebviewOptions {
        // Asked about every navigation before it starts.
        navigation_filter: Some(Arc::new(|request| !request.url.starts_with("http://"))),
        ..WebviewOptions::url("https://example.com")
    })
});

cx.subscribe_in(&webview, window, |_, webview, event, window, cx| match event {
    // Popups never open a window; send them to the system browser instead.
    WebviewEvent::PopupRequested { url, .. } => cx.open_url(url),
    WebviewEvent::JsDialog { id, kind: JsDialogKind::Confirm, message, .. } => {
        let (id, webview) = (*id, webview.downgrade());
        let answer = window.prompt(PromptLevel::Info, message, None, &["OK", "Cancel"], cx);
        cx.spawn(async move |_, cx| {
            let accept = answer.await == Ok(0);
            webview.update(cx, |webview, _| webview.resolve_dialog(id, accept, None)).ok();
        })
        .detach();
    }
    WebviewEvent::DownloadRequested { id, suggested_name, .. } => {
        // ... cx.prompt_for_new_path(...), then
        // webview.read(cx).resolve_download(id, Some(path)) — or None to cancel.
    }
    WebviewEvent::RenderProcessTerminated { .. } => webview.read(cx).reload(),
    _ => {}
})
.detach();
```

- **Answer each request once**, by the id its event carried: `resolve_dialog`,
  `resolve_file_dialog`, `resolve_download`. They return `false` for a request
  that is no longer waiting.
- **The page waits for the answer.** `alert()` and friends block the page's
  script until they are answered, so an application that ignores
  `JsDialog` leaves the page stuck; set `js_dialogs: DialogPolicy::AutoDismiss`
  to have every dialog dismissed on the spot instead. A dialog still open when
  the page navigates away is dismissed.
- **Nothing is left hanging when the webview goes.** Dropping it dismisses open
  dialogs, cancels file choosers, and cancels downloads that were never told
  where to go; anything the page asks for after that is turned down.
- **Popups never get a window.** By default they are blocked and reported;
  `popups: PopupPolicy::LoadInPlace` loads them in the same webview.
- **The navigation filter runs inside the browser engine**, synchronously, so
  keep it quick and do not reach back into the webview from it. A panicking
  filter counts as "no".
- **A crashed renderer leaves the last frame on screen.** `reload()` starts a
  new one and brings the page back.
- **Print through `print_to_pdf`.** The page's own `window.print()` puts the
  system print panel up as a window of its own, detached from the gpui window,
  and holds the page until it closes.

CEF calls these handlers on the main thread — with `external_message_pump` it
is CEF's UI thread — from inside the message pump. They only queue events, and
the pump emits them through gpui within a frame; the `resolve_*` calls answer
CEF right away, on the same thread.

### The demo (`examples/browser`)

`just run` launches it. The toolbar, address bar, and side panel are all drawn by
gpui; whether the history buttons are enabled and whether a load is in flight come
straight from CEF's `LoadHandler`.

- Type in the address bar and press Enter to navigate (input without a `.` is
  treated as a search term)
- Back, forward, and reload. The buttons grey out when there is no history
- A side panel button runs `eval` to inject JavaScript into the page (colour inversion)
- Toggling the side panel changes the webview's width, and CEF's render size follows

The webview is **clipped to rounded corners** and a **gpui element is drawn over
it** in the bottom right. A child-view backend can do neither, which is the reason
this crate exists.

The side panel's **Feature test page** has a button for everything the webview
hands to the application, and the demo answers each natively: gpui prompts for
`alert`, `confirm` and "leave this page?", a gpui overlay with a text field for
`prompt`, open and save panels for file choosers and downloads, the system
browser for popups, a Reload banner after a renderer crash, and a save panel for
printing to PDF. A navigation filter turns `blocked.example` away. The side panel
lists the latest events. `GPUI_CEF_START_URL=features` starts on that page.

## Upgrading from 0.1

- `WebviewEvent` is `#[non_exhaustive]`, and so are its struct-like variants:
  give a `match` a `_ => {}` arm, and match variants with `..`.
- `WebviewOptions` has new fields; build it with `WebviewOptions::url(..)` or
  `..Default::default()` as before.
- **Behaviour changes on macOS.** Popups no longer open a native window; they
  are blocked and reported as `PopupRequested`. JavaScript dialogs now wait for
  `resolve_dialog`: an application that does not handle `JsDialog` should set
  `js_dialogs: DialogPolicy::AutoDismiss`, or `alert()` will block the page.
  Downloads, which used to be cancelled, now wait for `resolve_download`.
- Page-to-app messages (`post_message_script`, `WebviewEvent::Message`) are
  unchanged. A page that worked around the missing pieces through them can drop
  the workarounds: an overridden `window.open` for `PopupRequested`, a
  `confirm()` bridged through messages for `JsDialog`, a heartbeat watchdog for
  `RenderProcessTerminated`, intercepted blob downloads for `DownloadRequested`,
  a hand-rolled file input for `FileDialog`.

## How frames get across, and what zero-copy still needs

With `shared_texture_enabled`, CEF hands over its output as a **shared IOSurface**.
gpui has a `surface()` element that composites a `CVPixelBuffer` directly as a
Metal texture, so bridging the two with `CVPixelBufferCreateWithIOSurface` should
make this zero-copy. That was the original plan, and **everything up to gpui does
work**: CEF to IOSurface to `CVPixelBuffer` to gpui is verified.

It stops at the last step on stock gpui. `surface()` hard-codes NV12
(`kCVPixelFormatType_420YpCbCr8BiPlanarFullRange`) and draws the Y and CbCr planes
through a YUV-to-RGB shader — it was written for Zed's screen sharing. CEF produces
BGRA, so passing it straight through trips an `assert_eq!` in `metal_renderer.rs`.

Measured cost of that copy, at 994x786 in a debug build: **1.2% CPU across all
processes when the page is idle, ~38% while the page repaints continuously at
60fps**. An application UI is idle almost all the time, so the copy rarely shows
up; a page running constant animation is where it does.

So the **default is the CPU copy path** (`WebviewOptions::accelerated == false`):
the BGRA that CEF's `OnPaint` returns is copied into a `RenderImage` and drawn with
`img()`. That is roughly 3.9MB per frame at 1200x820.

Making it zero-copy needs a BGRA branch in gpui — a pixel-format check in
`paint_surface` plus a shader that draws BGRA as-is. Only set `accelerated: true`
when running against a gpui that has one.

## Known limitations

- **No drag and drop** between the page and the rest of the application.
- **Folder uploads do not complete.** `<input webkitdirectory>` reaches the
  application as a `FileDialog` in `OpenFolder` mode, but after the answer
  Chromium asks the user to confirm the upload in a dialog a windowless browser
  cannot show, and the page never receives the files.
- **One download at a time without a click.** Chromium holds a page's second
  download until the user has interacted with the page again, and asks for
  permission in the meantime; Alloy-style CEF has no prompt for that, so a page
  that starts several downloads in a row without user input gets the first.
  Permission prompts in general (camera, microphone, notifications) are not
  handled yet, which with Alloy means they are not granted.
- **`window.print()`** opens the system print panel detached from the window;
  see above.
- **The CPU copy path is the default.** See the section above.
- The cursor the page asks for is forwarded to gpui, but CEF's custom cursors
  (`CT_CUSTOM`) fall back to an arrow.
- Input methods are wired up (see below) but the live round trip has not been
  confirmed by a human at a keyboard.

## Text input and input methods

Off-screen rendering leaves CEF with no window for the OS to attach an input
context to, so Chromium never sees the input method on its own. Sending key
events alone gets you ASCII and nothing else — Japanese, Chinese, Korean and
dead-key accents all arrive as *composition*, which the OS negotiates with the
focused view instead of delivering as keystrokes.

`platform/cef/ime.rs` implements gpui's `EntityInputHandler` against CEF's
`ImeSetComposition` / `ImeCommitText` / `ImeCancelComposition`, and uses the
rects from `OnImeCompositionRangeChanged` to place the candidate window. That
trait is written for an editor that owns its text, which this does not — but the
OS only ever asks about the text it is currently composing, and a mirror of just
that much is enough.

Text is sent only after the platform input handler accepts it. Ordinary BMP text
uses CEF `CHAR` events; composition commits and text containing astral Unicode
characters use `ImeCommitText`. A `CHAR` holds only one UTF-16 unit, so sending a
surrogate pair as two such events exposes ill-formed `beforeinput` / `input` data
to editors even when a plain input's eventual value looks correct. Astral text
is committed atomically, including when there is no active composition. The
native input probe checks each DOM input event as well as the final value.

GPUI owns the initial AppKit key and menu dispatch. After the renderer and
JavaScript have had a chance to handle a key, the CEF keyboard handler consumes
the remaining native fallback. Otherwise CEF sends even ordinary typing back
through `NSMenu.performKeyEquivalent:`, repeating menu validation and actions.
The existing focused-frame copy/cut/paste fallback still runs at this stage.

## Testing

```bash
cargo test --workspace
```

The tests cover the pure parts: the gpui-to-CEF input translation, the shared
state that decides when CEF has to be told about a resize, the popup layer
bookkeeping, the cursor mapping, the bookkeeping of requests waiting on the
application (ids, answering once, cancelling what is left), the event queue,
the translation of CEF's dialog, download and termination types, and the
demo's URL handling.

```bash
just e2e
```

checks the rest end to end on macOS. It bundles the demo, starts it with
`GPUI_CEF_DEMO_AUTO=1` — which answers every dialog, file chooser and download
on the spot rather than showing anything — on the feature test page, and clicks
through the page over the remote debugging port with a small CDP client
(`examples/browser/e2e.mjs`, Node 22, no dependencies). Each check looks at both
ends: the event the demo logged, and what the page or the disk saw after the
answer. It opens a real window, so it is not part of CI.

```bash
just input-e2e
```

checks the actual AppKit `NSTextInputClient` → GPUI → CEF path on macOS,
including accented characters, surrogate pairs, composition updates, commit,
finish and cancellation. The Node wrapper copies the bundle into a temporary
directory, changes its identifier, marks it background-only, signs it and
removes it afterwards. The test uses a disposable CEF profile, no debugging
port, a hidden window and logical browser focus without activating the app.
It invokes the native text-client callbacks and asserts the renderer's DOM
values and UTF-16 marked ranges. It also sends synthetic typing, Backspace, Left and
Delete NSEvents directly to its own view and checks typing, caret movement and
deletion of complete surrogate pairs.
Five additional key pairs go through the owned `NSWindow.sendEvent:` and its
first responder, with exact DOM `keydown`/`keyup` pairs checked as well as the
resulting text. A self-tested observer records `NSResponder.noResponderFor:`
in this disposable process and fails on any unexpected invocation. It suppresses
that method's default alert only in the test process; it does not change the
library or prove the absence of all warning sounds. An additional self-tested
`NSMenu.performKeyEquivalent:` observer preserves
AppKit's implementation and rejects renderer fallback into the native menu.
This checks replay behavior, not physical keyboard latency. The hidden test runs
in macOS CI. It does not post system keyboard events or switch the system input
source. Physical keyboard input, IME candidate UI, input latency and audio remain
unverified. A macOS WindowServer/Metal session is still required.

Anything that needs a live browser is exercised by running the demo instead.
CEF's remote debugging port is open, so `http://127.0.0.1:9229/json/list` plus
CDP can drive and inspect the page without looking at the screen — useful both in
a headless environment and for checking things that are hard to eyeball. That is
how the layout rect (994x786 inside a 1280x860 window) and the `<select>` popup
were verified.

## Building

### Prerequisites

| | Why | How |
|---|---|---|
| C++ compiler, cmake, ninja | building `libcef_dll_wrapper` | install with your OS package manager |
| CEF binaries (~500MB) | required by the CEF wrapper and runtime | downloaded automatically on the first build |

Set `CEF_PATH` to use an existing CEF distribution instead of the automatic
download. On macOS, **Xcode.app is not required**: this workspace enables
gpui's `runtime_shaders` feature and compiles shaders at runtime.

### Commands

```bash
just doctor   # check the prerequisites
just bundle   # build the platform bundle
just run      # bundle and launch
```

The bundle contains the CEF runtime and platform resources. macOS requires the
`.app` layout; Linux gets a `run.sh` launcher that sets the library search path.

## Implementation notes (the mines that were stepped on)

gpui and CEF both assume they own the foundations of the application, so making
them coexist takes some care. Every item below was found by crashing into it.

### Fighting over NSApplication

There is one `NSApp` per process, and it is instantiated as **whichever class
calls `sharedApplication` first**. Initializing CEF first produces a plain
`NSApplication`, and gpui then crashes because the instance it fetches through
`[GPUIApplication sharedApplication]` has none of its own ivars.

Fix: split initialization in two and let gpui create `NSApp` before CEF is
initialized (`init` then `Runtime::start`).

### CefAppProtocol

CEF and Chromium both CHECK
`[NSApp conformsToProtocol:@protocol(CefAppProtocol)]` and abort with a SIGTRAP if
it fails. gpui's class naturally knows nothing about it — and the protocol only
exists in headers, so it is not even registered with the Objective-C runtime.

Fix: register a protocol of the same name with `objc_allocateProtocol`, add
`isHandlingSendEvent` / `setHandlingSendEvent:` implementations, and swizzle
`sendEvent:` to raise the flag (`platform/cef/nsapp.rs`).

### The message loop

Calling `do_message_loop_work()` while gpui's `App` is borrowed lets gpui's paint
callbacks re-enter from the run loop Chromium spins, which panics with
`RefCell already borrowed`.

On macOS, gpui's foreground executor barely runs tasks while the app is idle
— measured at one tick every one to five seconds. Putting the pump there deadlocks
the whole thing: the pump does not run, so CEF produces no frames, so gpui does not
repaint, so the executor does not run.

Fix: macOS bypasses gpui and installs a `CFRunLoopTimer` on the main run loop at
60Hz (`platform/cef/pump.rs`). The callback comes straight from the run loop, so
nobody is borrowing gpui's `App`. CEF's `on_schedule_message_pump_work` callback
signals a thread-safe run-loop source that brings this timer forward for pending
work. Linux and Windows use a 16ms GPUI task to pump CEF on the UI thread.
Accelerated begin-frame requests are macOS-only.

`just input-e2e` also reports 100 host/renderer IPC round trips on macOS, after
10 warmups. See [message-pump measurements](docs/message-pump-performance.md)
for the method and baseline. This measures neither physical typing nor
document/display latency.

### The child process sandbox

The macOS helper initializes the CEF sandbox before launching renderer, GPU, and
network child processes. Linux and Windows use the browser executable itself for
CEF subprocesses.

### Runtime lifetime

Moving `Runtime` into the `Application::run` closure runs `cef_shutdown()` **the
moment the closure returns** — that is, right after startup — killing Chromium
mid-launch with a SIGTRAP.

Fix: make `Runtime` an `Rc`-backed shared handle and pass the closure a clone.

## Layout

```
crates/gpui-cef/
  src/platform/cef/     CEF off-screen rendering on macOS, Windows, and Linux
    mod.rs              init / Runtime / Webview / Element
    handlers.rs         CEF-side callbacks: rendering, display, loading, popups,
                        navigation, dialogs, file choosers, downloads, crashes
    pending.rs          requests waiting on the application's answer
    input.rs            gpui input events to CEF events
    nsapp.rs            retrofit CefAppProtocol onto NSApp (macOS)
    pump.rs             drive CEF from a CFRunLoopTimer (macOS)
    pump_non_macos.rs   pump CEF from a GPUI task (Linux and Windows)
    shared.rs           state shared between CEF and gpui
examples/browser/       demo app (main binary plus the CEF child-process helper),
                        its feature test page, and the end-to-end check
xtask/                  downloads are handled by cef-dll-sys; assembles platform bundles
```

## Not verified

- **Windows and macOS CEF builds.** CI covers both targets, but they cannot be
  built or launched from this Linux development environment.
- **Live Linux rendering and input** through the new CEF pump.
- **The demo's native answers.** `just e2e` answers automatically; that the
  gpui prompt, the open and save panels and the `prompt()` overlay come up has
  been seen in the window list, but clicking them has been left to a human.
- **Typing and IME.** Input translation is unit tested, but physical key events,
  CJK input, and platform-specific IME behavior need live verification.
- Scrolling direction and step size through CEF.
- More than one webview at a time. The message pump is shared across them by
  design, but only the single-webview case has been exercised.
- Screenshot-based visual verification of the demo.

## License

Licensed under either the [Apache License, Version 2.0](LICENSE-APACHE) or the
[MIT license](LICENSE-MIT), at your option, as declared by the workspace's Cargo
metadata. Dependencies, including the bundled browser engine, retain their own
licenses.
# gpui-browser
