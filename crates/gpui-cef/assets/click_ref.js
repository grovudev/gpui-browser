(() => {
  const el = document.querySelector(
    "[data-uno-ref=" + JSON.stringify(__UNO_REF__) + "]",
  );
  if (!el) return;
  el.scrollIntoView({ block: "center", behavior: "instant" });
  el.focus();
  for (const type of ["pointerdown", "mousedown", "pointerup", "mouseup"]) {
    el.dispatchEvent(
      new MouseEvent(type, {
        bubbles: true,
        cancelable: true,
        view: window,
      }),
    );
  }
  el.click();
})();
