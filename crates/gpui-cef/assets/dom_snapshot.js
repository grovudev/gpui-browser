try {
  const MAX_EL = 80,
    MAX_IMG = 30,
    MAX_IMG_SCAN = 300,
    MAX_TEXT = 2500,
    MAX_NODES = 3000;
  const vh = innerHeight,
    vw = innerWidth;
  const vis = { checkOpacity: true, checkVisibilityCSS: true };

  const sel =
    'a[href],button,input,textarea,select,summary,[role="button"],[role="link"],[role="checkbox"],[role="tab"],[role="menuitem"],[contenteditable="true"]';
  const picked = [];
  for (const el of document.querySelectorAll(sel)) {
    if (picked.length >= MAX_EL) break;
    if (el.disabled) continue;
    const type = (el.getAttribute("type") || "").toLowerCase();
    if (type === "hidden" || type === "password") continue;
    const r = el.getBoundingClientRect(); // один layout, дальше из кэша
    if (r.width <= 0 || r.height <= 0) continue;
    if (r.bottom < 0 || r.top > vh || r.right < 0 || r.left > vw) continue;
    if (!el.checkVisibility(vis)) continue;
    picked.push({ el, type });
  }

  const els = picked.map(({ el, type }, i) => {
    const tag = el.tagName.toLowerCase();
    const isField = tag === "input" || tag === "textarea" || tag === "select";
    const btnLike = isField && /^(button|submit|reset)$/.test(type);
    const text = (
      el.getAttribute("aria-label") ||
      (isField ? (btnLike ? el.value : "") : el.textContent) || // textContent: без layout
      ""
    )
      .trim()
      .slice(0, 80);
    const ph = el.getAttribute("placeholder");
    return {
      ref: "e" + (i + 1),
      tag,
      text,
      href: tag === "a" ? safeUrl(el.href) : undefined,
      placeholder: ph ? ph.slice(0, 80) : undefined,
      type: type || undefined,
    };
  });

  // видимый текст: TreeWalker с ранним выходом
  let text = "",
    nodes = 0,
    lastParent = null,
    lastOk = false;
  const tw = document.createTreeWalker(
    document.body || document.documentElement,
    NodeFilter.SHOW_TEXT,
  );
  while (tw.nextNode() && text.length < MAX_TEXT && nodes++ < MAX_NODES) {
    const n = tw.currentNode,
      p = n.parentElement;
    if (!p) continue;
    if (p !== lastParent) {
      lastParent = p;
      lastOk =
        !/^(SCRIPT|STYLE|NOSCRIPT|TEMPLATE)$/.test(p.tagName) &&
        p.checkVisibility(vis);
    }
    if (!lastOk) continue;
    const t = n.nodeValue.trim();
    if (t) text += t + " ";
  }

  const imgs = [];
  const imageList = document.images;
  const imageCount = Math.min(imageList.length, MAX_IMG_SCAN);
  for (let i = 0; i < imageCount; i++) {
    if (imgs.length >= MAX_IMG) break;
    const img = imageList[i];
    const r = img.getBoundingClientRect();
    if (r.width < 16 || r.height < 16 || r.bottom < 0 || r.top > vh) continue;
    const src = img.currentSrc || img.src;
    if (!src || src.startsWith("data:")) continue;
    imgs.push({
      src: safeUrl(src),
      alt: (img.alt || "").slice(0, 80) || undefined,
      width: Math.round(r.width),
      height: Math.round(r.height),
    });
  }

  const previousRefs = window.__unoRefNodes;
  if (Array.isArray(previousRefs)) {
    for (let i = 0; i < Math.min(previousRefs.length, MAX_EL); i++) {
      previousRefs[i]?.removeAttribute("data-uno-ref");
    }
  }
  window.__unoRefNodes = picked.map(({ el }, i) => {
    el.setAttribute("data-uno-ref", "e" + (i + 1));
    return el;
  });

  window.ipc.postMessage(
    JSON.stringify({
      kind: "dom",
      token: __UNO_IPC_TOKEN__,
      url: safeUrl(location.href),
      title: String(document.title || "").slice(0, 200),
      text: text.slice(0, MAX_TEXT),
      elements: els,
      images: imgs,
    }),
  );
} catch (e) {
  window.ipc.postMessage(
    JSON.stringify({
      kind: "error",
      token: __UNO_IPC_TOKEN__,
      message: String(e).slice(0, 200),
    }),
  );
}
