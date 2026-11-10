import { PoolPage } from "@/components/PoolPage";
import { pageMetadata } from "@/lib/pages";

export const metadata = pageMetadata("miners", "fr");

export default function Page() {
  return (
    <PoolPage lang="fr" page="miners">
      <section
        className={"miners-section wrap category-page"}
        id={"mineurs"}
        aria-labelledby={"miners-title"}
      >
        {"\n      "}
        <div className={"section-heading"}>
          <h1 id={"miners-title"}>{"Mineurs"}</h1>
          <button
            type={"button"}
            id={"refresh-miners"}
            className={"miners-refresh"}
          >
            {"Actualiser"}
          </button>
        </div>
        {"\n      "}
        <form id={"miner-search"} className={"miner-search"} noValidate>
          {"\n        "}
          <label htmlFor={"miner-lookup-address"}>{"Statistiques"}</label>
          {"\n        "}
          <div className={"miner-search-row"}>
            <input
              id={"miner-lookup-address"}
              placeholder={"bc1…, 1… ou 3…"}
              autoComplete={"off"}
              autoCapitalize={"off"}
              spellCheck={false}
              aria-describedby={"miner-search-message"}
            />
            <button type={"submit"} className={"button primary"}>
              {"Rechercher"}
            </button>
          </div>
          {"\n        "}
          <p
            id={"miner-search-message"}
            className={"miners-message"}
            role={"status"}
            aria-live={"polite"}
            hidden
          ></p>
          {"\n      "}
        </form>
        {"\n      "}
        <div id={"miner-statistics"} className={"miner-statistics"} hidden>
          {"\n        "}
          <div className={"miner-detail-heading"}>
            <div>
              <h3>{"Statistiques de l’appareil"}</h3>
              <code id={"selected-miner-address"}></code>
            </div>
            <span
              id={"miner-stats-updated"}
              className={"miners-updated"}
            ></span>
          </div>
          {"\n        "}
          <div className={"metrics-grid miner-summary"}>
            {"\n          "}
            <article className={"metric"}>
              <div className={"metric-label"}>{"Hashrate · 5 min"}</div>
              <p className={"metric-value"} id={"personal-hashrate"}>
                {"—"}
              </p>
            </article>
            {"\n          "}
            <article className={"metric"}>
              <div className={"metric-label"}>{"Connexions"}</div>
              <p className={"metric-value"} id={"personal-connections"}>
                {"—"}
              </p>
            </article>
            {"\n          "}
            <article className={"metric"}>
              <div className={"metric-label"}>{"Shares"}</div>
              <p className={"metric-value"} id={"personal-shares"}>
                {"—"}
              </p>
            </article>
            {"\n          "}
            <article className={"metric"}>
              <div className={"metric-label"}>{"Meilleure difficulté"}</div>
              <p className={"metric-value"} id={"personal-best"}>
                {"—"}
              </p>
            </article>
            {"\n        "}
          </div>
          {"\n        "}
          <div className={"miners-table-wrap"}>
            <table className={"miners-table"}>
              <thead>
                <tr>
                  <th scope={"col"}>{"Worker"}</th>
                  <th scope={"col"}>{"Appareil"}</th>
                  <th scope={"col"}>{"État"}</th>
                  <th scope={"col"}>{"Hashrate · 5 min"}</th>
                  <th scope={"col"}>{"Meilleure difficulté"}</th>
                </tr>
              </thead>
              <tbody id={"personal-workers"}></tbody>
            </table>
          </div>
          {"\n        "}
          <div
            className={"miners-pagination"}
            id={"personal-pagination"}
            hidden
          >
            <button type={"button"} id={"personal-previous"}>
              {"Précédent"}
            </button>
            <span id={"personal-page"}></span>
            <button type={"button"} id={"personal-next"}>
              {"Suivant"}
            </button>
          </div>
          {"\n      "}
        </div>
        {"\n      "}
        <div className={"online-miners"}>
          {"\n        "}
          <div className={"miner-detail-heading"}>
            <h3>{"Mineurs en ligne"}</h3>
            <span id={"online-count"} className={"miners-updated"}>
              {"—"}
            </span>
          </div>
          {"\n        "}
          <p id={"online-message"} className={"miners-message"} role={"status"}>
            {"Chargement…"}
          </p>
          {"\n        "}
          <div className={"miners-table-wrap"} id={"online-table-wrap"} hidden>
            <table className={"miners-table"}>
              <thead>
                <tr>
                  <th scope={"col"}>{"Appareil"}</th>
                  <th scope={"col"}>{"Adresse BTC"}</th>
                  <th scope={"col"}>{"Worker"}</th>
                  <th scope={"col"}>{"Hashrate · 5 min"}</th>
                  <th
                    scope={"col"}
                    title={"Meilleure difficulté signalée par CKPool"}
                  >
                    {"Meilleure difficulté"}
                  </th>
                </tr>
              </thead>
              <tbody id={"online-miners-body"}></tbody>
            </table>
          </div>
          {"\n        "}
          <div className={"online-footer"}>
            <span id={"online-updated"} className={"miners-updated"}></span>
            <div
              className={"miners-pagination"}
              id={"online-pagination"}
              hidden
            >
              <button type={"button"} id={"online-previous"}>
                {"Précédent"}
              </button>
              <span id={"online-page"}></span>
              <button type={"button"} id={"online-next"}>
                {"Suivant"}
              </button>
            </div>
          </div>
          {"\n      "}
        </div>
        {"\n    "}
      </section>
      {"\n"}
    </PoolPage>
  );
}
