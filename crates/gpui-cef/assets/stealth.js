(() => {
  try {
    Object.defineProperty(navigator, "webdriver", {
      get: () => undefined,
      configurable: true,
    });
    if (!navigator.languages || navigator.languages.length === 0) {
      Object.defineProperty(navigator, "languages", {
        get: () => ["en-US", "en"],
        configurable: true,
      });
    }
    Object.defineProperty(navigator, "hardwareConcurrency", {
      get: () => 8,
      configurable: true,
    });
    Object.defineProperty(navigator, "deviceMemory", {
      get: () => 8,
      configurable: true,
    });
    Object.defineProperty(navigator, "maxTouchPoints", {
      get: () => 0,
      configurable: true,
    });

    if (!window.chrome) {
      Object.defineProperty(window, "chrome", {
        value: { runtime: {}, app: { isInstalled: false } },
        configurable: true,
      });
    }

    const fakePlugins = [
      {
        name: "PDF Viewer",
        filename: "internal-pdf-viewer",
        description: "Portable Document Format",
      },
      {
        name: "Chrome PDF Viewer",
        filename: "mhjfbmdgcfjbbpaeojofohoefgiehjai",
        description: "Portable Document Format",
      },
    ];
    Object.defineProperty(navigator, "plugins", {
      get: () => fakePlugins,
      configurable: true,
    });

    if (navigator.permissions && navigator.permissions.query) {
      const origQuery = navigator.permissions.query.bind(navigator.permissions);
      navigator.permissions.query = (params) =>
        params && params.name === "notifications"
          ? Promise.resolve({ state: Notification.permission })
          : origQuery(params);
    }

    const patchWebGL = (proto) => {
      if (!proto || !proto.getParameter) return;
      const orig = proto.getParameter;
      proto.getParameter = function (param) {
        if (param === 37445) return "Intel Inc.";
        if (param === 37446) return "Intel Iris OpenGL Engine";
        return orig.apply(this, arguments);
      };
    };
    patchWebGL(
      window.WebGLRenderingContext && window.WebGLRenderingContext.prototype,
    );
    patchWebGL(
      window.WebGL2RenderingContext && window.WebGL2RenderingContext.prototype,
    );

    if (window.outerWidth === 0)
      Object.defineProperty(window, "outerWidth", {
        get: () => window.innerWidth || 1280,
      });
    if (window.outerHeight === 0)
      Object.defineProperty(window, "outerHeight", {
        get: () => window.innerHeight || 820,
      });
  } catch (_) {}
})();
