import type { ReactNode } from "react";
import type { Language, PageName } from "@/lib/pages";
import { Header, Footer } from "./Navigation";
import { Icons } from "./Icons";
import { PageScripts } from "./PageScripts";

export function PoolPage({
  lang,
  page,
  children,
}: {
  lang: Language;
  page: PageName;
  children: ReactNode;
}) {
  return (
    <>
      <Icons />
      <a className="skip-link" href="#main">
        {lang === "en" ? "Skip to content" : "Aller au contenu"}
      </a>
      <Header lang={lang} page={page} />
      <main
        id="main"
        className={page === "api" ? "api-docs wrap category-page" : undefined}
      >
        {children}
      </main>
      <Footer lang={lang} page={page} />
      <div
        className="toast"
        id="toast"
        role="status"
        aria-live="polite"
        hidden
      />
      <noscript>
        <p className="noscript-message">
          {lang === "en"
            ? "Enable JavaScript for live readings and miner settings. "
            : "Activez JavaScript pour les relevés et les réglages du mineur. "}
          Stratum V1 : stratum.bdl-pool.com:39991 · Stratum V2 :
          stratum.bdl-pool.com:36809.
        </p>
      </noscript>
      <PageScripts lang={lang} page={page} />
    </>
  );
}
