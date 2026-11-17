(() => {
  try {
    const apply = () => {
      let meta = document.querySelector('meta[name="viewport"]');
      if (!meta) {
        meta = document.createElement("meta");
        meta.name = "viewport";
        (document.head || document.documentElement).appendChild(meta);
      }
      meta.content = "width=1280";
    };
    if (document.documentElement) apply();
    document.addEventListener("DOMContentLoaded", apply, { once: true });
  } catch (_) {}
})();
