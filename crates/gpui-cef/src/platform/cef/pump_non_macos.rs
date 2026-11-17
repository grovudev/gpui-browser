use std::time::Duration;

use cef::ImplBrowserHost as _;
use gpui::{Context, Task, WeakEntity};

use super::{Webview, shared::Shared};

const PUMP_INTERVAL: Duration = Duration::from_millis(16);

pub(crate) fn schedule_work(_delay_ms: i64) {}

pub(crate) struct Registration {
    _task: Task<()>,
}

pub(crate) fn register(
    shared: std::rc::Rc<Shared>,
    _accelerated: bool,
    webview: WeakEntity<Webview>,
    cx: &Context<Webview>,
) -> Registration {
    let task = cx.spawn(async move |_, cx| {
        let mut stale = Vec::new();
        loop {
            cx.background_executor().timer(PUMP_INTERVAL).await;
            if shared.is_closed() {
                break;
            }

            cef::do_message_loop_work();

            if shared.take_resized() {
                if let Some(host) = shared.host() {
                    host.was_resized();
                }
            }

            shared.drain_stale_cpu_frames(&mut stale);
            let events = shared.take_events();
            let dirty = shared.take_dirty();
            if stale.is_empty() && events.is_empty() && !dirty {
                continue;
            }

            if webview
                .update(cx, |_, cx| {
                    for frame in stale.drain(..) {
                        cx.drop_image(frame, None);
                    }
                    for event in events {
                        cx.emit(event);
                    }
                    if dirty {
                        cx.notify();
                    }
                })
                .is_err()
            {
                break;
            }
        }
    });
    Registration { _task: task }
}
