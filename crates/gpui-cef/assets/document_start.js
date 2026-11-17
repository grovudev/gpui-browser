(() => {
  __UNO_NETWORK_TAP__;
  Object.defineProperty(window, "ipc", {
    configurable: true,
    value: {
      postMessage(message) {
        console.debug("gpui-cef:message:" + String(message));
      },
    },
  });
  __UNO_STEALTH__;
  __UNO_VIEWPORT__;
  try {
    Object.defineProperty(navigator, "userAgent", {
      get: () => __UNO_USER_AGENT__,
      configurable: true,
    });
  } catch (_) {}
})();
