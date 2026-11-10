"use client";

import { useEffect, useState } from "react";
import type { Language } from "@/lib/pages";

export function ThemeToggle({ lang }: { lang: Language }) {
  const [dark, setDark] = useState(true);
  useEffect(() => {
    const savedDark = document.documentElement.dataset.theme !== "light";
    setDark(savedDark);
    document
      .querySelector('meta[name="theme-color"]')
      ?.setAttribute("content", savedDark ? "#080f16" : "#f6f7fc");
  }, []);
  const label =
    lang === "en"
      ? dark
        ? "Switch to light theme"
        : "Switch to dark theme"
      : dark
        ? "Activer le thème clair"
        : "Activer le thème sombre";

  function toggle() {
    const nextDark = !dark;
    document.documentElement.dataset.theme = nextDark ? "dark" : "light";
    try {
      localStorage.setItem("bdl-pool:theme", nextDark ? "dark" : "light");
    } catch {
      /* Theme still applies. */
    }
    document
      .querySelector('meta[name="theme-color"]')
      ?.setAttribute("content", nextDark ? "#080f16" : "#f6f7fc");
    setDark(nextDark);
    document.dispatchEvent(new Event("bdl-theme-change"));
  }

  return (
    <button
      id="theme-toggle"
      type="button"
      className="theme-toggle"
      title={label}
      aria-label={label}
      aria-pressed={dark}
      onClick={toggle}
    >
      <svg className="icon sun-icon" viewBox="0 0 24 24" aria-hidden="true">
        <circle cx="12" cy="12" r="4" />
        <path d="M12 2v2m0 16v2M4.93 4.93l1.42 1.42m11.3 11.3 1.42 1.42M2 12h2m16 0h2M4.93 19.07l1.42-1.42m11.3-11.3 1.42-1.42" />
      </svg>
      <svg className="icon moon-icon" viewBox="0 0 24 24" aria-hidden="true">
        <path d="M20.2 15.3A8.6 8.6 0 0 1 8.7 3.8 8.8 8.8 0 1 0 20.2 15.3Z" />
      </svg>
    </button>
  );
}
