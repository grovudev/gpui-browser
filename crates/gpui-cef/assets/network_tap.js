(() => {
  if (window.__unoNetInstalled) return;
  window.__unoNetInstalled = true;
  const MAX_NET = 80;
  const MAX_ERR = 30;
  const MAX_PREVIEW = 4000;
  const MAX_FORM_FIELDS = 50;
  const MAX_ACTIVE_READERS = 16;
  const REQUEST_PREVIEW_HEADERS = [
    "accept",
    "content-type",
    "x-requested-with",
  ];
  const RESPONSE_PREVIEW_HEADERS = [
    "content-type",
    "content-length",
    "cache-control",
    "etag",
    "x-request-id",
    "request-id",
    "x-correlation-id",
    "traceparent",
  ];
  const net = (window.__unoNetLog = []);
  const errs = (window.__unoConsoleLog = []);
  const pendingCaptures = (window.__unoNetPending = new Set());
  let activeReaders = 0;
  const safeUrl = (value) => {
    try {
      const parsed = new URL(String(value || ""), location.href);
      parsed.username = "";
      parsed.password = "";
      for (const name of parsed.searchParams.keys()) {
        if (
          /(token|key|auth|secret|password|signature|session|cookie|credential|jwt|code)/i.test(
            name,
          )
        ) {
          parsed.searchParams.set(name, "[REDACTED]");
        }
      }
      return parsed.href.slice(0, 300);
    } catch (_) {
      return "[unparseable URL]";
    }
  };
  const pickHeaders = (headers, allowed) => {
    const result = {};
    try {
      const source = new Headers(headers || {});
      for (const name of allowed) {
        const value = source.get(name);
        if (value !== null) result[name] = value.slice(0, 200);
      }
    } catch (_) {}
    return Object.keys(result).length ? result : undefined;
  };
  const bodyPreview = (body) => {
    if (typeof body === "string") return body.slice(0, MAX_PREVIEW);
    if (body instanceof URLSearchParams) {
      const entries = [];
      let size = 0;
      for (const [key, value] of body) {
        if (entries.length >= MAX_FORM_FIELDS) break;
        const encoded = new URLSearchParams([[key, value]]).toString();
        const separator = entries.length ? 1 : 0;
        if (size + separator + encoded.length > MAX_PREVIEW) break;
        entries.push(encoded);
        size += separator + encoded.length;
      }
      return entries.join("&");
    }
    if (body instanceof FormData) {
      const entries = [];
      let size = 2;
      for (const [key, value] of body.entries()) {
        if (entries.length >= MAX_FORM_FIELDS) break;
        const item = [
          String(key).slice(0, 200),
          typeof value === "string"
            ? value.slice(0, 1000)
            : `[file: ${String(value.name || "").slice(0, 200)}; ${value.size} bytes]`,
        ];
        const encoded = JSON.stringify(item);
        const separator = entries.length ? 1 : 0;
        if (size + separator + encoded.length > MAX_PREVIEW) break;
        entries.push(encoded);
        size += separator + encoded.length;
      }
      return `[${entries.join(",")}]`;
    }
    if (body instanceof Blob)
      return `[blob: ${body.type || "unknown"}, ${body.size} bytes]`;
    if (body instanceof ArrayBuffer)
      return `[binary body: ${body.byteLength} bytes]`;
    if (ArrayBuffer.isView(body))
      return `[binary body: ${body.byteLength} bytes]`;
    return undefined;
  };
  const readLimited = async (stream, limit) => {
    if (!stream || activeReaders >= MAX_ACTIVE_READERS) return undefined;
    activeReaders++;
    let timeout;
    try {
      const reader = stream.getReader();
      const decoder = new TextDecoder();
      let text = "";
      let bytesRead = 0;
      const maxBytes = limit * 4;
      const timeoutPromise = new Promise((resolve) => {
        timeout = setTimeout(() => resolve({ timedOut: true }), 750);
      });
      while (text.length < limit) {
        const next = await Promise.race([reader.read(), timeoutPromise]);
        if (next.timedOut) {
          reader.cancel().catch(() => {});
          break;
        }
        const { value, done } = next;
        if (done) {
          text += decoder.decode();
          break;
        }
        const byteCount = Math.min(value.byteLength, maxBytes - bytesRead);
        bytesRead += byteCount;
        text += decoder.decode(value.subarray(0, byteCount), {
          stream: true,
        });
        if (text.length >= limit || byteCount < value.byteLength) {
          text = text.slice(0, limit);
          reader.cancel().catch(() => {});
          break;
        }
      }
      return text.slice(0, limit);
    } catch (_) {
    } finally {
      clearTimeout(timeout);
      activeReaders--;
    }
    return undefined;
  };
  const trackCapture = (entry, requestPromise, responsePromise) => {
    if (requestPromise === undefined && responsePromise === undefined) return;
    let tracked;
    tracked = Promise.all([requestPromise, responsePromise])
      .then(([request_preview, preview]) => {
        if (request_preview !== undefined)
          entry.request_preview = request_preview;
        if (preview !== undefined) entry.preview = preview;
      })
      .catch(() => {})
      .finally(() => {
        pendingCaptures.delete(tracked);
      });
    pendingCaptures.add(tracked);
  };
  const requestBodyPreview = (input, init) => {
    if (init && Object.prototype.hasOwnProperty.call(init, "body")) {
      const preview = bodyPreview(init.body);
      return preview === undefined ? undefined : Promise.resolve(preview);
    }
    if (
      input instanceof Request &&
      input.body &&
      activeReaders < MAX_ACTIVE_READERS
    ) {
      try {
        return readLimited(input.clone().body, MAX_PREVIEW);
      } catch (_) {}
    }
    return undefined;
  };
  const pushNet = (entry) => {
    if (net.length >= MAX_NET) net.shift();
    net.push(entry);
  };
  const pushErr = (level, msg) => {
    if (errs.length >= MAX_ERR) errs.shift();
    errs.push({ level, message: String(msg || "").slice(0, 300) });
  };

  if (window.fetch) {
    const origFetch = window.fetch.bind(window);
    window.fetch = async function (input, init) {
      const t0 = performance.now();
      const url =
        typeof input === "string" ? input : (input && input.url) || "";
      const method = (init && init.method) || (input && input.method) || "GET";
      const headers =
        init && init.headers !== undefined
          ? init.headers
          : input instanceof Request
            ? input.headers
            : undefined;
      const requestPreviewPromise = requestBodyPreview(input, init);
      try {
        const res = await origFetch(input, init);
        let responsePreviewPromise;
        try {
          const ct = res.headers.get("content-type") || "";
          const contentLength = Number(res.headers.get("content-length"));
          if (
            activeReaders < MAX_ACTIVE_READERS &&
            (!Number.isFinite(contentLength) ||
              contentLength <= MAX_PREVIEW * 4) &&
            /json|text|xml|javascript|event-stream/i.test(ct)
          ) {
            responsePreviewPromise = readLimited(res.clone().body, MAX_PREVIEW);
          }
        } catch (_) {}
        const entry = {
          kind: "fetch",
          method: String(method).toUpperCase(),
          url: safeUrl(url),
          status: String(res.status),
          duration_ms: Math.round(performance.now() - t0),
          request_headers: pickHeaders(headers, REQUEST_PREVIEW_HEADERS),
          response_headers: pickHeaders(res.headers, RESPONSE_PREVIEW_HEADERS),
        };
        pushNet(entry);
        trackCapture(entry, requestPreviewPromise, responsePreviewPromise);
        return res;
      } catch (e) {
        const entry = {
          kind: "fetch",
          method: String(method).toUpperCase(),
          url: safeUrl(url),
          status: "error",
          duration_ms: Math.round(performance.now() - t0),
          preview: String(e).slice(0, 200),
          request_headers: pickHeaders(headers, REQUEST_PREVIEW_HEADERS),
        };
        pushNet(entry);
        trackCapture(entry, requestPreviewPromise, Promise.resolve(undefined));
        throw e;
      }
    };
  }

  if (window.XMLHttpRequest) {
    const origOpen = XMLHttpRequest.prototype.open;
    const origSend = XMLHttpRequest.prototype.send;
    const origSetRequestHeader = XMLHttpRequest.prototype.setRequestHeader;
    XMLHttpRequest.prototype.open = function (method, url) {
      this.__unoMeta = {
        method: String(method || "GET").toUpperCase(),
        url: safeUrl(url),
      };
      return origOpen.apply(this, arguments);
    };
    XMLHttpRequest.prototype.setRequestHeader = function (name, value) {
      const key = String(name || "").toLowerCase();
      if (REQUEST_PREVIEW_HEADERS.includes(key)) {
        this.__unoHeaders = this.__unoHeaders || {};
        this.__unoHeaders[key] = String(value || "").slice(0, 200);
      }
      return origSetRequestHeader.apply(this, arguments);
    };
    XMLHttpRequest.prototype.send = function (body) {
      const t0 = performance.now();
      const meta = this.__unoMeta || { method: "GET", url: "" };
      const request_preview = bodyPreview(body);
      this.addEventListener(
        "loadend",
        () => {
          let preview;
          try {
            const contentLength = Number(
              this.getResponseHeader("content-length"),
            );
            if (
              (!Number.isFinite(contentLength) ||
                contentLength <= MAX_PREVIEW * 4) &&
              typeof this.responseText === "string"
            )
              preview = this.responseText.slice(0, MAX_PREVIEW);
          } catch (_) {}
          const response_headers = {};
          for (const name of RESPONSE_PREVIEW_HEADERS) {
            try {
              const value = this.getResponseHeader(name);
              if (value !== null) response_headers[name] = value.slice(0, 200);
            } catch (_) {}
          }
          pushNet({
            kind: "xhr",
            method: meta.method,
            url: meta.url,
            status: String(this.status || "error"),
            duration_ms: Math.round(performance.now() - t0),
            preview,
            request_preview,
            request_headers: this.__unoHeaders,
            response_headers: Object.keys(response_headers).length
              ? response_headers
              : undefined,
          });
        },
        { once: true },
      );
      return origSend.apply(this, arguments);
    };
  }

  if (window.EventSource) {
    const OrigES = window.EventSource;
    window.EventSource = function (url, cfg) {
      const es = new OrigES(url, cfg);
      const u = safeUrl(url);
      pushNet({ kind: "sse", url: u, status: "open" });
      es.addEventListener("message", (ev) => {
        pushNet({
          kind: "sse",
          url: u,
          status: "message",
          preview: String(ev.data || "").slice(0, 300),
        });
      });
      es.addEventListener("error", () => {
        pushNet({ kind: "sse", url: u, status: "error" });
      });
      return es;
    };
    window.EventSource.prototype = OrigES.prototype;
  }

  if (window.WebSocket) {
    const OrigWS = window.WebSocket;
    window.WebSocket = function (url, protocols) {
      const ws =
        protocols !== undefined ? new OrigWS(url, protocols) : new OrigWS(url);
      const u = safeUrl(url);
      pushNet({ kind: "ws", url: u, status: "connecting" });
      ws.addEventListener("open", () =>
        pushNet({ kind: "ws", url: u, status: "open" }),
      );
      ws.addEventListener("message", (ev) => {
        pushNet({
          kind: "ws",
          url: u,
          status: "message",
          preview: String(ev.data || "").slice(0, 300),
        });
      });
      ws.addEventListener("close", (ev) =>
        pushNet({ kind: "ws", url: u, status: "closed:" + ev.code }),
      );
      ws.addEventListener("error", () =>
        pushNet({ kind: "ws", url: u, status: "error" }),
      );
      return ws;
    };
    window.WebSocket.prototype = OrigWS.prototype;
  }

  const origErr = console.error.bind(console);
  console.error = (...args) => {
    pushErr("error", args.map(String).join(" "));
    origErr(...args);
  };
  window.addEventListener("error", (ev) => pushErr("uncaught", ev.message));
  window.addEventListener("unhandledrejection", (ev) =>
    pushErr("rejection", ev.reason),
  );
})();
