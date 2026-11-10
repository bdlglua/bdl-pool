import {
  pageNames,
  pages,
  route,
  type Language,
  type PageName,
} from "@/lib/pages";
import { ThemeToggle } from "./ThemeToggle";

function ExternalIcon() {
  return (
    <svg className="icon" aria-hidden="true">
      <use href="#i-external" />
    </svg>
  );
}

function Brand({ lang }: { lang: Language }) {
  return (
    <a
      className="brand"
      href={route("home", lang)}
      aria-label={`BDL Pool, ${lang === "en" ? "home" : "accueil"}`}
    >
      <img src="/assets/brand.svg" width="34" height="34" alt="" />
      <span>
        BDL<span className="brand-light">Pool</span>
      </span>
    </a>
  );
}

function Links({ lang, page }: { lang: Language; page: PageName }) {
  // Full document links preserve the independent lifecycles of the existing
  // live-data tools while their views are rendered and exported by Next.js.
  return (
    <>
      {pageNames.map((key) => (
        <a
          key={key}
          href={route(key, lang)}
          className={key === page ? "active" : undefined}
          aria-current={key === page ? "page" : undefined}
        >
          {pages[key].labels[lang === "en" ? 1 : 0]}
        </a>
      ))}
      <a href="https://miner.bdl-pool.com">BDLMiner</a>
    </>
  );
}

export function Header({ lang, page }: { lang: Language; page: PageName }) {
  const en = lang === "en";
  return (
    <header className="site-header">
      <div className="header-inner wrap">
        <Brand lang={lang} />
        <nav
          id="main-nav"
          className="main-nav"
          aria-label={en ? "Main navigation" : "Navigation principale"}
        >
          <Links lang={lang} page={page} />
        </nav>
        <div className="header-actions">
          <ThemeToggle lang={lang} />
          <a
            className="language-switch"
            href={route(page, en ? "fr" : "en")}
            lang={en ? "fr" : "en"}
            hrefLang={en ? "fr" : "en"}
            aria-label={en ? "Passer en français" : "Switch to English"}
          >
            {en ? "FR" : "EN"}
          </a>
          <a
            className="header-github"
            href="https://github.com/bdlglua/bdl-pool"
            target="_blank"
            rel="noopener noreferrer"
          >
            GitHub <ExternalIcon />
          </a>
          <button
            className="menu-toggle"
            type="button"
            aria-label={en ? "Open menu" : "Ouvrir le menu"}
            aria-controls="main-nav"
            aria-expanded="false"
          >
            <span />
            <span />
            <span />
          </button>
        </div>
      </div>
    </header>
  );
}

export function Footer({ lang, page }: { lang: Language; page: PageName }) {
  return (
    <footer className="site-footer">
      <div className="footer-inner wrap">
        <nav
          className="footer-pages"
          aria-label={lang === "en" ? "All pages" : "Toutes les pages"}
        >
          <Links lang={lang} page={page} />
        </nav>
        <div className="footer-top">
          <Brand lang={lang} />
          <p>{lang === "en" ? "Solo Bitcoin pool" : "Pool Bitcoin solo"}</p>
          <a
            href="https://github.com/bdlglua/bdl-pool"
            target="_blank"
            rel="noopener noreferrer"
          >
            GitHub <ExternalIcon />
          </a>
        </div>
        <div className="footer-bottom">
          <span>© BDL Pool</span>
          <a
            href="https://github.com/bdlglua"
            target="_blank"
            rel="noopener noreferrer"
          >
            bdlglua
          </a>
        </div>
      </div>
    </footer>
  );
}
