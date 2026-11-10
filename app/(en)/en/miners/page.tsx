import { PoolPage } from "@/components/PoolPage";
import { pageMetadata } from "@/lib/pages";

export const metadata = pageMetadata("miners", "en");

export default function Page() {
  return (
    <PoolPage lang="en" page="miners">
      <section
        className={"miners-section wrap category-page"}
        id={"mineurs"}
        aria-labelledby={"miners-title"}
      >
        {"\n      "}
        <div className={"section-heading"}>
          <h1 id={"miners-title"}>{"Miners"}</h1>
          <button
            type={"button"}
            id={"refresh-miners"}
            className={"miners-refresh"}
          >
            {"Refresh"}
          </button>
        </div>
        {"\n      "}
        <form id={"miner-search"} className={"miner-search"} noValidate>
          {"\n        "}
          <label htmlFor={"miner-lookup-address"}>
            {"Statistics by Bitcoin address"}
          </label>
          {"\n        "}
          <div className={"miner-search-row"}>
            <input
              id={"miner-lookup-address"}
              placeholder={"bc1…, 1… or 3…"}
              autoComplete={"off"}
              autoCapitalize={"off"}
              spellCheck={false}
              aria-describedby={"miner-search-message"}
            />
            <button type={"submit"} className={"button primary"}>
              {"Search"}
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
              <h3>{"Address statistics"}</h3>
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
              <div className={"metric-label"}>{"Connections"}</div>
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
              <div className={"metric-label"}>{"Best difficulty"}</div>
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
                  <th scope={"col"}>{"Device"}</th>
                  <th scope={"col"}>{"Status"}</th>
                  <th scope={"col"}>{"Hashrate · 5 min"}</th>
                  <th scope={"col"}>{"Best difficulty"}</th>
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
              {"Previous"}
            </button>
            <span id={"personal-page"}></span>
            <button type={"button"} id={"personal-next"}>
              {"Next"}
            </button>
          </div>
          {"\n      "}
        </div>
        {"\n      "}
        <div className={"online-miners"}>
          {"\n        "}
          <div className={"miner-detail-heading"}>
            <h3>{"Online miners"}</h3>
            <span id={"online-count"} className={"miners-updated"}>
              {"—"}
            </span>
          </div>
          {"\n        "}
          <p id={"online-message"} className={"miners-message"} role={"status"}>
            {"Loading…"}
          </p>
          {"\n        "}
          <div className={"miners-table-wrap"} id={"online-table-wrap"} hidden>
            <table className={"miners-table"}>
              <thead>
                <tr>
                  <th scope={"col"}>{"Device"}</th>
                  <th scope={"col"}>{"BTC address"}</th>
                  <th scope={"col"}>{"Worker"}</th>
                  <th scope={"col"}>{"Hashrate · 5 min"}</th>
                  <th
                    scope={"col"}
                    title={"Best difficulty reported by BDLPool"}
                  >
                    {"Best difficulty"}
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
                {"Previous"}
              </button>
              <span id={"online-page"}></span>
              <button type={"button"} id={"online-next"}>
                {"Next"}
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
