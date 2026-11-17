(async () => {
  try {
    const includePreview = __UNO_INCLUDE_PREVIEW__;
    if (includePreview) {
      await Promise.race([
        Promise.allSettled(window.__unoNetPending || []),
        new Promise((resolve) => setTimeout(resolve, 800)),
      ]);
    }
    const net = window.__unoNetLog || [];
    const errs = window.__unoConsoleLog || [];
    const requests = net.slice(includePreview ? -20 : -60).map((entry) => {
      if (includePreview) return { ...entry };
      const {
        preview,
        request_preview,
        request_headers,
        response_headers,
        ...safe
      } = entry;
      return safe;
    });
    const payload = {
      kind: "network",
      token: __UNO_IPC_TOKEN__,
      url: safeUrl(location.href, 500),
      requests,
      console_errors: errs.slice(-20),
    };
    const encoder = new TextEncoder();
    let serialized = JSON.stringify(payload);
    const maxBytes = 220 * 1024;
    while (encoder.encode(serialized).length > maxBytes) {
      let changed = false;
      for (const request of requests) {
        for (const name of ["preview", "request_preview"]) {
          const value = request[name];
          if (typeof value !== "string") continue;
          request[name] =
            value.length > 64
              ? value.slice(0, Math.floor(value.length / 2))
              : undefined;
          changed = true;
        }
      }
      if (!changed && requests.length > 1) {
        requests.shift();
      } else if (!changed) {
        break;
      }
      serialized = JSON.stringify(payload);
    }
    if (__UNO_CLEAR_NET__) {
      net.length = 0;
      errs.length = 0;
    }
    window.ipc.postMessage(serialized);
  } catch (e) {}
})();
