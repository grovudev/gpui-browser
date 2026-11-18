use std::{
    sync::atomic::{AtomicI64, Ordering},
    time::Duration,
};

use cef::ImplBrowserHost as _;
use gpui::{Context, Task, WeakEntity};

use super::{shared::Shared, Webview};

const FAST_PUMP_INTERVAL: Duration = Duration::from_millis(4);
const ACTIVE_PUMP_INTERVAL: Duration = Duration::from_millis(16);
const IDLE_PUMP_INTERVAL: Duration = Duration::from_millis(100);

static NEXT_WORK_DELAY_MS: AtomicI64 = AtomicI64::new(16);

pub(crate) fn schedule_work(delay_ms: i64) {
    let clamped = delay_ms.clamp(0, 100);
    let _ = NEXT_WORK_DELAY_MS.fetch_min(clamped, Ordering::Release);
}

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
            let scheduled_ms = NEXT_WORK_DELAY_MS.swap(100, Ordering::AcqRel);
            let interval = if scheduled_ms <= 4 {
                FAST_PUMP_INTERVAL
            } else if shared.is_visible() || shared.is_loading() || scheduled_ms <= 16 {
                ACTIVE_PUMP_INTERVAL
            } else {
                IDLE_PUMP_INTERVAL
            };
            cx.background_executor().timer(interval).await;
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
