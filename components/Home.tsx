import type { Language } from "@/lib/pages";
import { route } from "@/lib/pages";
import { PoolPage } from "./PoolPage";

function Icon({ name }: { name: string }) {
  return <svg className="icon" aria-hidden="true"><use href={`#i-${name}`} /></svg>;
}

export function Home({ lang }: { lang: Language }) {
  const en = lang === "en";
  const shortcuts = [
    ["chip", "Stratum V1", route("connect", lang) + "?protocol=v1"],
    ["network", "Stratum V2", route("connect", lang) + "?protocol=v2"],
    ["chart", en ? "Statistics" : "Statistiques", route("statistics", lang)],
    ["miners", en ? "Miners" : "Mineurs", route("miners", lang)],
    ["code", "API", route("api", lang)],
    ["chip", "BDLMiner", "https://miner.bdl-pool.com"],
  ];

  return <PoolPage lang={lang} page="home">
    <section className="hero wrap hero-stage" id="accueil" aria-labelledby="hero-title">
      <div className="hero-copy">
        <h1 id="hero-title">
          {en ? "Solo Bitcoin mining," : "Bitcoin en solo,"}<br />
          <span className="hero-accent">{en ? "with BDL Pool" : "avec BDL Pool"}</span>
        </h1>
        <p className="hero-description">{en ? "Stratum V1 and Stratum V2 connections" : "Connexion Stratum V1 et Stratum V2"}</p>
        <div className="hero-actions">
          <a href={route("connect", lang)} className="button primary">
            {en ? "Connect a miner" : "Connecter un appareil"}<Icon name="arrow" />
          </a>
          <a href={route("statistics", lang)} className="text-link">
            {en ? "Statistics" : "Statistiques"}<Icon name="arrow" />
          </a>
        </div>
      </div>

      <div className="hero-visual">
        <article className="panel node-panel" aria-labelledby="home-node-title">
          <div className="panel-heading">
            <span className="node-heading-icon"><Icon name="chip" /></span>
            <h2 id="home-node-title">{en ? "Bitcoin node" : "Nœud Bitcoin"}</h2>
          </div>
          <div className="node-status">
            <span className="badge amber" id="node-badge">{en ? "Sync" : "Synchronisation"}</span>
            <span className="quiet-label">Mainnet</span>
          </div>
          <p className="sync-value" id="sync-value">—<span> %</span></p>
          <div className="progress-track" id="sync-progress" role="progressbar"
            aria-label={en ? "Estimated verification progress" : "Progression estimée de la vérification"}
            aria-valuemin={0} aria-valuemax={100} aria-valuenow={0}>
            <div id="sync-fill" />
          </div>
          <div className="node-numbers">
            <span>{en ? "Validated blocks" : "Blocs validés"}<strong id="node-blocks">—</strong></span>
            <span>{en ? "Known headers" : "En-têtes connus"}<strong id="node-headers">—</strong></span>
          </div>
          <p className="node-note" id="node-note">{en ? "BDLPool will start after syncing." : "BDLPool démarrera après la synchronisation."}</p>
        </article>
        <div className="home-status">
          <div className="heading-actions">
            <span className="badge neutral" id="pool-badge">{en ? "Loading…" : "Chargement…"}</span>
            <button type="button" id="refresh-status" className="icon-button status-refresh"
              aria-label={en ? "Refresh status" : "Actualiser le suivi"} aria-busy="false">
              <Icon name="refresh" /><span>{en ? "Refresh" : "Actualiser"}</span>
            </button>
          </div>
          <p className="data-notice" id="data-notice">{en ? "Loading latest snapshot…" : "Chargement du dernier relevé…"}</p>
        </div>
      </div>
    </section>
    <section className="home-categories wrap">
      <div className="pulse-shortcuts" aria-label={en ? "Quick links" : "Accès rapides"}>
        {shortcuts.map(([icon, label, href]) => <a href={href} key={label}><Icon name={icon} /><span>{label}</span></a>)}
      </div>
    </section>
  </PoolPage>;
}
