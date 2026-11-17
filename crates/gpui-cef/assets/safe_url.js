const safeUrl = (value, maxLength = 300) => {
  try {
    const parsed = new URL(String(value || ""), location.href);
    parsed.username = "";
    parsed.password = "";
    parsed.hash = "";
    for (const name of parsed.searchParams.keys()) {
      if (
        /(token|key|auth|secret|password|signature|session|cookie|credential|jwt|code)/i.test(
          name,
        )
      ) {
        parsed.searchParams.set(name, "[REDACTED]");
      }
    }
    return parsed.href.slice(0, maxLength);
  } catch (_) {
    return "[unparseable URL]";
  }
};
