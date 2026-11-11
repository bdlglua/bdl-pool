"use client";

import Script from "next/script";
import { useEffect, useState } from "react";
import { route, type Language, type PageName } from "@/lib/pages";

export function PageScripts({
  page,
  lang,
}: {
  page: PageName;
  lang: Language;
}) {
  const [ready, setReady] = useState(false);
  const [loaded, setLoaded] = useState(0);
  useEffect(() => {
    document.body.dataset.page = page;
    document.body.dataset.home = route("home", lang);
    document.body.dataset.legacy = JSON.stringify(
      page === "home"
        ? {
            dashboard: route("statistics", lang),
            mineurs: route("miners", lang),
            connexion: route("connect", lang),
            faq: route("faq", lang),
          }
        : {},
    );
    setReady(true);
  }, [page, lang]);

  const scripts = ["/config.js", "/assets/coins.js", "/assets/site.js", "/assets/navigation.js"];
  if (page === "statistics")
    scripts.push("/assets/vendor/chart.umd.min.js", "/assets/pulse-chart.js");
  if (["home", "statistics", "connect", "faq"].includes(page))
    scripts.push("/assets/app.js");
  if (page === "miners") scripts.push("/assets/miners.js");
  if (page === "api") scripts.push("/assets/api.js");

  // Initialise DOM-based tools only after React hydrates, and load config/Chart
  // before their consumers. Normal anchor navigation starts a fresh document.
  return ready ? (
    <>
      {scripts.slice(0, loaded + 1).map((src, index) => (
        <Script
          key={src}
          src={src}
          strategy="afterInteractive"
          onReady={() => setLoaded((value) => Math.max(value, index + 1))}
        />
      ))}
    </>
  ) : null;
}
