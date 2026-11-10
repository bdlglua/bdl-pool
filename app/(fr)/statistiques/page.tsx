import { PoolPage } from "@/components/PoolPage";
import { pageMetadata } from "@/lib/pages";

export const metadata = pageMetadata("statistics", "fr");

export default function Page() {
  return (
    <PoolPage lang="fr" page="statistics">
      <section
        className={"dashboard-section wrap category-page"}
        id={"dashboard"}
        aria-labelledby={"dashboard-title"}
      >
        {"\n      "}
        <div className={"section-heading"}>
          <div>
            <h1 id={"dashboard-title"}>{"Statistiques"}</h1>
          </div>
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
        </div>
        {"\n      "}
        <p className={"data-notice"} id={"data-notice"}>
          {"Chargement du dernier relevé…"}
        </p>
        {"\n\n      "}
        <div className={"metrics-grid"}>
          {"\n        "}
          <article className={"metric"}>
            <div className={"metric-label"}>{"Hashrate"}</div>
            <p className={"metric-value"} id={"metric-hashrate"}>
              {"—"}
            </p>
          </article>
          {"\n        "}
          <article className={"metric"}>
            <div className={"metric-label"}>{"Workers"}</div>
            <p className={"metric-value"} id={"metric-workers"}>
              {"—"}
            </p>
          </article>
          {"\n        "}
          <article className={"metric"}>
            <div className={"metric-label"}>{"Meilleure share"}</div>
            <p className={"metric-value"} id={"metric-best"}>
              {"—"}
            </p>
          </article>
          {"\n        "}
          <article className={"metric"}>
            <div className={"metric-label"}>{"Blocs trouvés"}</div>
            <p className={"metric-value"} id={"metric-blocks"}>
              {"—"}
            </p>
          </article>
          {"\n      "}
        </div>
        {"\n\n\n\n      "}
        <div className={"overview-grid stats-grid"}>
          {"\n        "}
          <article className={"panel activity-panel"}>
            <div className={"panel-heading"}>
              <h3>{"Hashrate"}</h3>
            </div>
            <div className={"chart-area"} id={"chart-area"}>
              <div className={"empty-chart"} id={"chart-empty"}>
                <strong>{"Aucune mesure"}</strong>
              </div>
              <div
                className={"pulse-chart-frame"}
                id={"pool-chart-frame"}
                hidden
              >
                <canvas
                  id={"cryptoChart"}
                  role={"img"}
                  aria-label={"Historique du hashrate du pool"}
                  aria-describedby={"sample-count"}
                  hidden
                ></canvas>
                <svg
                  id={"hash-chart"}
                  viewBox={"0 0 640 170"}
                  role={"img"}
                  aria-label={"Évolution du hashrate mesuré"}
                  {...{ hidden: "" }}
                ></svg>
              </div>
            </div>
            <div className={"panel-foot"}>
              <span>
                <span className={"dot muted-dot"}></span>{" "}
                <span id={"activity-state"}>{"Aucune mesure reçue"}</span>
              </span>
              <span id={"sample-count"}>{"Données non disponibles"}</span>
            </div>
          </article>
          {"\n\n      "}
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
        {"\n    "}
      </section>
      {"\n"}
    </PoolPage>
  );
}
