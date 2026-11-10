import { PoolPage } from "@/components/PoolPage";
import { pageMetadata } from "@/lib/pages";

export const metadata = pageMetadata("home", "fr");

export default function Page() {
  return (
    <PoolPage lang="fr" page="home">
      <section
        className={"hero wrap hero-stage"}
        id={"accueil"}
        aria-labelledby={"hero-title"}
      >
        {"\n      "}
        <div className={"hero-copy"}>
          {"\n        "}
          <h1 id={"hero-title"}>
            {"Bitcoin en solo,"}
            <br />
            <span className={"hero-accent"}>{"avec BDL Pool."}</span>
          </h1>
          {"\n        "}
          <p className={"hero-description"}>
            {"Connexion Stratum V1 et Stratum V2."}
          </p>
          {"\n        "}
          <div className={"hero-actions"}>
            <a href={"/connexion/"} className={"button primary"}>
              {"Connecter un appareil"}
              <svg className={"icon"} aria-hidden={"true"}>
                <use href={"#i-arrow"} />
              </svg>
            </a>
            <a href={"/statistiques/"} className={"text-link"}>
              {"Statistiques"}
              <svg className={"icon"} aria-hidden={"true"}>
                <use href={"#i-arrow"} />
              </svg>
            </a>
          </div>
          {"\n        "}
          <div className={"home-status"}>
            <div className={"heading-actions"}>
              <span className={"badge neutral"} id={"pool-badge"}>
                {"Chargement…"}
              </span>
              <button
                type={"button"}
                id={"refresh-status"}
                className={"icon-button status-refresh"}
                aria-label={"Actualiser le suivi"}
                aria-busy={"false"}
              >
                <svg
                  className={"icon"}
                  viewBox={"0 0 24 24"}
                  aria-hidden={"true"}
                >
                  <use href={"#i-refresh"} />
                </svg>
                <span>{"Actualiser"}</span>
              </button>
            </div>
            <p className={"data-notice"} id={"data-notice"}>
              {"Chargement du dernier relevé…"}
            </p>
          </div>
          {"\n      "}
        </div>
        {"\n      "}
        <div className={"hero-visual"}>
          <article className={"panel node-panel"}>
            <div className={"panel-heading"}>
              <span className={"node-heading-icon"}>
                <img
                  src={"/assets/crypgo/icon-bitcoin.svg"}
                  width={"19"}
                  height={"24"}
                  alt={""}
                />
              </span>
              <h3>{"Nœud Bitcoin"}</h3>
            </div>
            <div className={"node-status"}>
              <span className={"badge amber"} id={"node-badge"}>
                {"Synchronisation"}
              </span>
              <span className={"quiet-label"}>{"MAINNET"}</span>
            </div>
            <p className={"sync-value"} id={"sync-value"}>
              {"—"}
              <span>{" %"}</span>
            </p>
            <div
              className={"progress-track"}
              id={"sync-progress"}
              role={"progressbar"}
              aria-label={"Progression estimée de la vérification"}
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={0}
            >
              <div id={"sync-fill"}></div>
            </div>
            <div className={"node-numbers"}>
              <span>
                {"Blocs validés"}
                <strong id={"node-blocks"}>{"—"}</strong>
              </span>
              <span>
                {"En-têtes connus"}
                <strong id={"node-headers"}>{"—"}</strong>
              </span>
            </div>
            <p className={"node-note"} id={"node-note"}>
              {"BDLPool démarrera après la synchronisation."}
            </p>
          </article>
        </div>
        {"\n      "}
        <img
          className={"hero-network"}
          src={"/assets/pulse-orbit.svg"}
          width={"360"}
          height={"340"}
          alt={""}
          aria-hidden={"true"}
        />
        {"\n    "}
      </section>
      {"\n"}
      <section className={"home-categories wrap"}>
        <div className={"pulse-shortcuts"} aria-label={"Accès rapides"}>
          <a href={"/connexion/?protocol=v1"}>
            <img
              src={"/assets/pulse-hardware.svg"}
              width={"72"}
              height={"72"}
              alt={""}
            />
            <span>{"Stratum V1"}</span>
          </a>
          <a href={"/connexion/?protocol=v2"}>
            <img
              src={"/assets/pulse-link.svg"}
              width={"72"}
              height={"72"}
              alt={""}
            />
            <span>{"Stratum V2"}</span>
          </a>
          <a href={"/statistiques/"}>
            <img
              src={"/assets/pulse-chart.svg"}
              width={"72"}
              height={"72"}
              alt={""}
            />
            <span>{"Statistiques"}</span>
          </a>
          <a href={"/mineurs/"}>
            <img
              src={"/assets/pulse-shield.svg"}
              width={"72"}
              height={"72"}
              alt={""}
            />
            <span>{"Workers"}</span>
          </a>
          <a href={"/api/"}>
            <img
              src={"/assets/pulse-flash.svg"}
              width={"72"}
              height={"72"}
              alt={""}
            />
            <span>{"API"}</span>
          </a>
          <a href={"https://miner.bdl-pool.com"}>
            <img
              src={"/assets/pulse-network.svg"}
              width={"72"}
              height={"72"}
              alt={""}
            />
            <span>{"BDLMiner"}</span>
          </a>
        </div>
      </section>
      {"\n"}
    </PoolPage>
  );
}
