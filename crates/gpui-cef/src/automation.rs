const MOBILE_USER_AGENT: &str = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36";

#[cfg(target_os = "macos")]
const DESKTOP_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
#[cfg(target_os = "windows")]
const DESKTOP_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
const DESKTOP_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

const DOCUMENT_START_JS: &str = include_str!("../assets/document_start.js");
const DESKTOP_VIEWPORT_JS: &str = include_str!("../assets/desktop_viewport.js");
const STEALTH_INIT_JS: &str = include_str!("../assets/stealth.js");
const NETWORK_TAP_INIT_JS: &str = include_str!("../assets/network_tap.js");
const SAFE_URL_JS: &str = include_str!("../assets/safe_url.js");
const EXTRACT_DOM_JS_BODY: &str = include_str!("../assets/dom_snapshot.js");
const EXTRACT_NETWORK_JS_BODY: &str = include_str!("../assets/network_snapshot.js");
const CLICK_REF_JS: &str = include_str!("../assets/click_ref.js");
const TYPE_REF_JS: &str = include_str!("../assets/type_ref.js");
const SCROLL_BY_JS: &str = include_str!("../assets/scroll_by.js");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewportMode {
    #[default]
    Mobile,
    Desktop,
}

pub fn user_agent(viewport_mode: ViewportMode) -> &'static str {
    match viewport_mode {
        ViewportMode::Mobile => MOBILE_USER_AGENT,
        ViewportMode::Desktop => DESKTOP_USER_AGENT,
    }
}

pub fn document_start_script(stealth_enabled: bool, viewport_mode: ViewportMode) -> String {
    let stealth = if stealth_enabled { STEALTH_INIT_JS } else { "" };
    let viewport = if viewport_mode == ViewportMode::Desktop {
        DESKTOP_VIEWPORT_JS
    } else {
        ""
    };
    let user_agent = serde_json::to_string(user_agent(viewport_mode))
        .expect("serializing a static user agent cannot fail");
    DOCUMENT_START_JS
        .replace("__UNO_NETWORK_TAP__", NETWORK_TAP_INIT_JS)
        .replace("__UNO_STEALTH__", stealth)
        .replace("__UNO_VIEWPORT__", viewport)
        .replace("__UNO_USER_AGENT__", &user_agent)
}

pub fn dom_snapshot(token: &str) -> Option<String> {
    let token = serde_json::to_string(token).ok()?;
    let body = EXTRACT_DOM_JS_BODY.replace("__UNO_IPC_TOKEN__", &token);
    Some(format!("(() => {{\n{SAFE_URL_JS}\n{body}\n}})();"))
}

pub fn network_log(token: &str, clear: bool, include_preview: bool) -> Option<String> {
    let token = serde_json::to_string(token).ok()?;
    let body = EXTRACT_NETWORK_JS_BODY
        .replace("__UNO_IPC_TOKEN__", &token)
        .replace("__UNO_CLEAR_NET__", if clear { "true" } else { "false" })
        .replace(
            "__UNO_INCLUDE_PREVIEW__",
            if include_preview { "true" } else { "false" },
        );
    Some(format!("(() => {{\n{SAFE_URL_JS}\n{body}\n}})();"))
}

pub fn click_ref(ref_id: &str) -> Option<String> {
    let reference = serde_json::to_string(ref_id).ok()?;
    Some(CLICK_REF_JS.replace("__UNO_REF__", &reference))
}

pub fn type_ref(ref_id: &str, text: &str, submit: bool) -> Option<String> {
    let reference = serde_json::to_string(ref_id).ok()?;
    let text = serde_json::to_string(text).ok()?;
    Some(
        TYPE_REF_JS
            .replace("__UNO_REF__", &reference)
            .replace("__UNO_TEXT__", &text)
            .replace("__UNO_SUBMIT__", if submit { "true" } else { "false" }),
    )
}

pub fn scroll_by(delta_y: i32) -> String {
    SCROLL_BY_JS.replace("__UNO_DELTA__", &delta_y.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        ViewportMode, click_ref, document_start_script, dom_snapshot, network_log, scroll_by,
        type_ref,
    };

    #[test]
    fn generated_scripts_replace_all_template_parameters() {
        let scripts = [
            document_start_script(true, ViewportMode::Desktop),
            dom_snapshot("token").unwrap(),
            network_log("token", true, true).unwrap(),
            click_ref("e1").unwrap(),
            type_ref("e1", "safe value", true).unwrap(),
            scroll_by(-120),
        ];
        for script in scripts {
            assert!(!script.contains("__UNO_"));
        }
    }

    #[test]
    fn snapshots_keep_their_outer_wrapper_and_redact_urls() {
        for script in [
            dom_snapshot("token").unwrap(),
            network_log("token", false, false).unwrap(),
        ] {
            assert!(script.starts_with("(() => {\n"));
            assert!(script.trim_end().ends_with("})();"));
            assert!(script.contains("parsed.hash ="));
            assert!(script.contains("[REDACTED]"));
        }
        assert!(
            network_log("token", false, false)
                .unwrap()
                .contains("url: safeUrl(location.href, 500)")
        );
    }

    #[test]
    fn user_input_is_inserted_as_json_literals() {
        let script = type_ref("e'1", "'); alert(1); //", false).unwrap();
        assert!(script.contains(r#"JSON.stringify("e'1")"#));
        assert!(script.contains(r#"setter.call(el, "'); alert(1); //")"#));
    }
}
