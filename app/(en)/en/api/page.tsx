import { PoolPage } from "@/components/PoolPage";
import { pageMetadata } from "@/lib/pages";

export const metadata = pageMetadata("api", "en");

export default function Page() {
  return (
    <PoolPage lang="en" page="api">
      <h1>{"BDL Pool API"}</h1>
      <p className={"api-intro"}>
        {"Pool, Bitcoin address and worker statistics."}
      </p>
      {"\n"}
      <section className={"api-base"} aria-labelledby={"api-base-title"}>
        <h2 id={"api-base-title"}>{"Base URL"}</h2>
        <div className={"api-base-url"}>
          <code id={"api-base-url"}>
            {"https://api.bdl-pool.com:36018/api/v1"}
          </code>
          <button
            type={"button"}
            className={"api-copy"}
            data-copy={"api-base-url"}
          >
            {"Copy"}
          </button>
        </div>
        <div className={"api-actions"}>
          <button
            type={"button"}
            id={"api-health-test"}
            className={"button primary"}
          >
            {"Test API"}
          </button>
          <a href={"/api/openapi.json"}>{"OpenAPI contract ↗"}</a>
        </div>
        <p id={"api-message"} role={"status"} aria-live={"polite"} hidden></p>
        <pre id={"api-health-result"} hidden></pre>
      </section>
      {"\n"}
      <section aria-labelledby={"api-routes-title"}>
        <h2 id={"api-routes-title"}>{"Endpoints"}</h2>
        <table>
          <thead>
            <tr>
              <th scope={"col"}>{"URL"}</th>
              <th scope={"col"}>{"Response"}</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td>
                <code>{"GET /online?limit=50&offset=0"}</code>
              </td>
              <td>
                {
                  "Active connections, shortened addresses and advertised devices."
                }
              </td>
            </tr>
            <tr>
              <td>
                <code>{"GET /health"}</code>
              </td>
              <td>{"API process health"}</td>
            </tr>
            <tr>
              <td>
                <code>{"GET /pool"}</code>
              </td>
              <td>{"Global pool and Bitcoin node statistics"}</td>
            </tr>
            <tr>
              <td>
                <code>{"GET /history"}</code>
              </td>
              <td>{"Hashrate history"}</td>
            </tr>
            <tr>
              <td>
                <code>{"GET /stratum"}</code>
              </td>
              <td>{"Address, ports and V2 public key"}</td>
            </tr>
            <tr>
              <td>
                <code>{"GET /miners/{address}"}</code>
              </td>
              <td>{"Statistics for one Bitcoin address"}</td>
            </tr>
            <tr>
              <td>
                <code>{"GET /miners/{address}/workers?limit=50&offset=0"}</code>
              </td>
              <td>{"Workers for one address"}</td>
            </tr>
            <tr>
              <td>
                <code>{"GET /miners/{address}/workers/{worker}"}</code>
              </td>
              <td>{"Statistics for one worker"}</td>
            </tr>
            <tr>
              <td>
                <code>{"GET /openapi.json"}</code>
              </td>
              <td>{"OpenAPI 3.1 specification"}</td>
            </tr>
          </tbody>
        </table>
      </section>
      {"\n"}
      <section aria-labelledby={"api-examples-title"}>
        <h2 id={"api-examples-title"}>{"Examples"}</h2>
        <div className={"api-code-heading"}>
          <h3>{"cURL"}</h3>
          <button type={"button"} className={"api-copy"} data-copy={"api-curl"}>
            {"Copy"}
          </button>
        </div>
        <pre>
          <code id={"api-curl"}>
            {
              'curl https://api.bdl-pool.com:36018/api/v1/pool\ncurl "https://api.bdl-pool.com:36018/api/v1/miners/YOUR_BITCOIN_ADDRESS/workers?limit=50&offset=0"'
            }
          </code>
        </pre>
        <div className={"api-code-heading"}>
          <h3>{"JavaScript"}</h3>
          <button
            type={"button"}
            className={"api-copy"}
            data-copy={"api-javascript"}
          >
            {"Copy"}
          </button>
        </div>
        <pre>
          <code id={"api-javascript"}>
            {
              'const base = "https://api.bdl-pool.com:36018/api/v1";\nconst address = "YOUR_BITCOIN_ADDRESS";\nconst response = await fetch(`${base}/miners/${encodeURIComponent(address)}`);\nif (!response.ok) throw new Error(`HTTP ${response.status}`);\nconst miner = await response.json();'
            }
          </code>
        </pre>
        {"\n"}
        <div className={"api-code-heading"}>
          <h3>{"TypeScript"}</h3>
          <button
            type={"button"}
            className={"api-copy"}
            data-copy={"api-typescript"}
          >
            {"Copy"}
          </button>
        </div>
        <pre>
          <code id={"api-typescript"}>
            {
              'type Health = { apiVersion: "1"; status: "ok" };\n\nconst response = await fetch("https://api.bdl-pool.com:36018/api/v1/health");\nif (!response.ok) throw new Error(`HTTP ${response.status}`);\nconst health: Health = await response.json();\nconsole.log(health.status);'
            }
          </code>
        </pre>
        <div className={"api-code-heading"}>
          <h3>{"Rust (reqwest + serde)"}</h3>
          <button type={"button"} className={"api-copy"} data-copy={"api-rust"}>
            {"Copy"}
          </button>
        </div>
        <pre>
          <code id={"api-rust"}>
            {
              'use serde::Deserialize;\n\n#[derive(Deserialize)]\nstruct Health {\n\t#[serde(rename = "apiVersion")]\n\tapi_version: String,\n\tstatus: String,\n}\n\nfn main() -> Result<(), Box<dyn std::error::Error>> {\n\tlet response = reqwest::blocking::get(\n\t\t"https://api.bdl-pool.com:36018/api/v1/health",\n\t)?.error_for_status()?;\n\tlet health: Health = response.json()?;\n\tprintln!("{} ({})", health.status, health.api_version);\n\tOk(())\n}'
            }
          </code>
        </pre>
        <p>
          {"Required crates: "}
          <code>{"reqwest"}</code>
          {" (with the "}
          <code>{"blocking"}</code>
          {" and "}
          <code>{"json"}</code>
          {" features) and "}
          <code>{"serde"}</code>
          {" (with the "}
          <code>{"derive"}</code>
          {" feature)."}
        </p>
        <div className={"api-code-heading"}>
          <h3>{"Go"}</h3>
          <button type={"button"} className={"api-copy"} data-copy={"api-go"}>
            {"Copy"}
          </button>
        </div>
        <pre>
          <code id={"api-go"}>
            {
              'package main\n\nimport (\n\t"encoding/json"\n\t"fmt"\n\t"net/http"\n)\n\nfunc main() {\n\tresponse, err := http.Get("https://api.bdl-pool.com:36018/api/v1/health")\n\tif err != nil {\n\t\tpanic(err)\n\t}\n\tdefer response.Body.Close()\n\tif response.StatusCode != http.StatusOK {\n\t\tpanic(response.Status)\n\t}\n\n\tvar health struct {\n\t\tAPIVersion string `json:"apiVersion"`\n\t\tStatus     string `json:"status"`\n\t}\n\tif err := json.NewDecoder(response.Body).Decode(&health); err != nil {\n\t\tpanic(err)\n\t}\n\tfmt.Printf("%s (%s)\\n", health.Status, health.APIVersion)\n}'
            }
          </code>
        </pre>
        <p>
          <a
            href={"https://github.com/bdlglua/bdl-pool/tree/main/api/examples"}
          >
            {"Runnable TypeScript, Rust, and Go clients"}
          </a>
          {" · "}
          <a href={"https://github.com/bdlglua/bdl-pool/tree/main/api/tools"}>
            {"Monitoring, Discord bot, and Prometheus exporter"}
          </a>
        </p>
      </section>
      {"\n"}
      <section aria-labelledby={"api-format-title"}>
        <h2 id={"api-format-title"}>{"Data format"}</h2>
        <dl className={"api-facts"}>
          <dt>{"Presence"}</dt>
          <dd>
            {
              "Connections are checked every 15 seconds. available=false means that their state is unconfirmed."
            }
          </dd>
          <dt>{"Device"}</dt>
          <dd>
            {
              "Hardware model advertised in the Stratum user-agent, otherwise the software name. Missing models remain null."
            }
          </dd>
          <dt>{"Updates"}</dt>
          <dd>
            {
              "Global statistics update every five minutes. Worker statistics follow CKPool."
            }
          </dd>
          <dt>{"Dates"}</dt>
          <dd>
            {"updatedAt is the measurement time. stale marks an old reading."}
          </dd>
          <dt>{"Hashrate"}</dt>
          <dd>
            {"All hashrateHps values are H/s. Unknown values remain null."}
          </dd>
          <dt>{"Addresses"}</dt>
          <dd>
            {
              "Use a Bitcoin mainnet address without a worker suffix. CKPool creates its statistics after its first recorded share."
            }
          </dd>
          <dt>{"Workers"}</dt>
          <dd>
            {
              "limit defaults to 50, up to 200. Use offset for pagination. Full worker names avoid ambiguity."
            }
          </dd>
          <dt>{"Availability"}</dt>
          <dd>
            {
              "readyAtLastCheck refers to the last Stratum V1 check. It does not confirm a V2 connection."
            }
          </dd>
          <dt>{"Access"}</dt>
          <dd>
            {
              "Read-only, without authentication. GET, HEAD and OPTIONS; public CORS. No directory of miner addresses is exposed."
            }
          </dd>
        </dl>
      </section>
      {"\n"}
      <section aria-labelledby={"api-errors-title"}>
        <h2 id={"api-errors-title"}>{"HTTP codes"}</h2>
        <dl className={"api-facts api-errors"}>
          <dt>
            <code>{"200"}</code>
          </dt>
          <dd>{"Data is available; check stale for older readings."}</dd>
          <dt>
            <code>{"304"}</code>
          </dt>
          <dd>{"Unchanged response for this ETag."}</dd>
          <dt>
            <code>{"400"}</code>
          </dt>
          <dd>{"Invalid address or parameters."}</dd>
          <dt>
            <code>{"404"}</code>
          </dt>
          <dd>{"Endpoint, address or worker not found."}</dd>
          <dt>
            <code>{"405"}</code>
          </dt>
          <dd>{"Write method not allowed."}</dd>
          <dt>
            <code>{"503"}</code>
          </dt>
          <dd>{"Statistics source unavailable."}</dd>
        </dl>
      </section>
      {"\n"}
    </PoolPage>
  );
}
