(() => {
  "use strict";

  const root = document.documentElement;
  const storageKey = "bdl-pool:theme";
  const english = root.lang === "en";
  const svgNamespace = "http://www.w3.org/2000/svg";

  function icon(className, shape) {
    const svg = document.createElementNS(svgNamespace, "svg");
    svg.setAttribute("class", `icon ${className}`);
    svg.setAttribute("viewBox", "0 0 24 24");
    svg.setAttribute("aria-hidden", "true");
    if (shape === "sun") {
      const circle = document.createElementNS(svgNamespace, "circle");
      circle.setAttribute("cx", "12");
      circle.setAttribute("cy", "12");
      circle.setAttribute("r", "4");
      svg.appendChild(circle);
      const rays = document.createElementNS(svgNamespace, "path");
      rays.setAttribute("d", "M12 2v2m0 16v2M4.93 4.93l1.42 1.42m11.3 11.3 1.42 1.42M2 12h2m16 0h2M4.93 19.07l1.42-1.42m11.3-11.3 1.42-1.42");
      svg.appendChild(rays);
    } else {
      const crescent = document.createElementNS(svgNamespace, "path");
      crescent.setAttribute("d", "M20.2 15.3A8.6 8.6 0 0 1 8.7 3.8 8.8 8.8 0 1 0 20.2 15.3Z");
      svg.appendChild(crescent);
    }
    return svg;
  }

  function updateButton() {
    const button = document.getElementById("theme-toggle");
    if (!button) return;
    const dark = root.dataset.theme === "dark";
    const label = dark
      ? (english ? "Switch to light theme" : "Activer le thème clair")
      : (english ? "Switch to dark theme" : "Activer le thème sombre");
    button.setAttribute("aria-label", label);
    button.setAttribute("title", label);
    button.setAttribute("aria-pressed", String(dark));
    document.querySelector('meta[name="theme-color"]')?.setAttribute("content", dark ? "#070913" : "#f6f7fc");
  }

  function addButton() {
    const container = document.querySelector(".header-actions, .api-header-links");
    if (!container || document.getElementById("theme-toggle")) return;
    const button = document.createElement("button");
    button.id = "theme-toggle";
    button.type = "button";
    button.className = "theme-toggle";
    button.append(icon("sun-icon", "sun"), icon("moon-icon", "moon"));
    const languageLink = container.querySelector(".language-switch");
    container.insertBefore(button, languageLink);
    updateButton();
  }

  try {
    root.dataset.theme = localStorage.getItem(storageKey) === "light" ? "light" : "dark";
  } catch {
    root.dataset.theme = "dark";
  }

  document.addEventListener("DOMContentLoaded", addButton, { once: true });
  document.addEventListener("click", (event) => {
    if (!(event.target instanceof Element) || !event.target.closest("#theme-toggle")) return;
    root.dataset.theme = root.dataset.theme === "dark" ? "light" : "dark";
    try { localStorage.setItem(storageKey, root.dataset.theme); } catch { /* Theme still applies for this page view. */ }
    updateButton();
    document.dispatchEvent(new Event("bdl-theme-change"));
  });
})();