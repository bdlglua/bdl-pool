import { PoolPage } from "@/components/PoolPage";
import { pageMetadata } from "@/lib/pages";

export const metadata = pageMetadata("api", "fr");

export default function Page() {
  return (
    <PoolPage lang="fr" page="api">
      <h1>{"API BDL Pool"}</h1>
      <p className={"api-intro"}>
        {"Statistiques du pool, des adresses Bitcoin et des workers."}
      </p>
      {"\n"}
      <section className={"api-base"} aria-labelledby={"api-base-title"}>
        <h2 id={"api-base-title"}>{"Adresse de base"}</h2>
        <div className={"api-base-url"}>
          <code id={"api-base-url"}>
            {"https://api.bdl-pool.com:36018/api/v1"}
          </code>
          <button
            type={"button"}
            className={"api-copy"}
            data-copy={"api-base-url"}
          >
            {"Copier"}
          </button>
        </div>
        <div className={"api-actions"}>
          <button
            type={"button"}
            id={"api-health-test"}
            className={"button primary"}
          >
            {"Tester l’API"}
          </button>
          <a href={"/api/openapi.json"}>{"Contrat OpenAPI ↗"}</a>
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
              <th scope={"col"}>{"Réponse"}</th>
            </tr>
          </thead>
          <tbody>
            <tr>
              <td>
                <code>{"GET /online?limit=50&offset=0"}</code>
              </td>
              <td>
                {"Connexions actives, adresses abrégées et appareils annoncés."}
              </td>
            </tr>
            <tr>
              <td>
                <code>{"GET /health"}</code>
              </td>
              <td>{"État du service API"}</td>
            </tr>
            <tr>
              <td>
                <code>{"GET /pool"}</code>
              </td>
              <td>{"Statistiques globales et nœud Bitcoin"}</td>
            </tr>
            <tr>
              <td>
                <code>{"GET /history"}</code>
              </td>
              <td>{"Historique du hashrate"}</td>
            </tr>
            <tr>
              <td>
                <code>{"GET /stratum"}</code>
              </td>
              <td>{"Adresse, ports et clé publique V2"}</td>
            </tr>
            <tr>
              <td>
                <code>{"GET /miners/{address}"}</code>
              </td>
              <td>{"Statistiques d’une adresse Bitcoin"}</td>
            </tr>
            <tr>
              <td>
                <code>{"GET /miners/{address}/workers?limit=50&offset=0"}</code>
              </td>
              <td>{"Workers d’une adresse"}</td>
            </tr>
            <tr>
              <td>
                <code>{"GET /miners/{address}/workers/{worker}"}</code>
              </td>
              <td>{"Détail d’un worker"}</td>
            </tr>
            <tr>
              <td>
                <code>{"GET /openapi.json"}</code>
              </td>
              <td>{"Spécification OpenAPI 3.1"}</td>
            </tr>
          </tbody>
        </table>
      </section>
      {"\n"}
      <section aria-labelledby={"api-examples-title"}>
        <h2 id={"api-examples-title"}>{"Exemples"}</h2>
        <div className={"api-code-heading"}>
          <h3>{"cURL"}</h3>
          <button type={"button"} className={"api-copy"} data-copy={"api-curl"}>
            {"Copier"}
          </button>
        </div>
        <pre>
          <code id={"api-curl"}>
            {
              'curl https://api.bdl-pool.com:36018/api/v1/pool\ncurl "https://api.bdl-pool.com:36018/api/v1/miners/VOTRE_ADRESSE_BTC/workers?limit=50&offset=0"'
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
            {"Copier"}
          </button>
        </div>
        <pre>
          <code id={"api-javascript"}>
            {
              'const base = "https://api.bdl-pool.com:36018/api/v1";\nconst address = "VOTRE_ADRESSE_BTC";\nconst response = await fetch(`${base}/miners/${encodeURIComponent(address)}`);\nif (!response.ok) throw new Error(`HTTP ${response.status}`);\nconst miner = await response.json();'
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
            {"Copier"}
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
            {"Copier"}
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
          {"Crates nécessaires : "}
          <code>{"reqwest"}</code>
          {" (fonctionnalités "}
          <code>{"blocking"}</code>
          {" et "}
          <code>{"json"}</code>
          {") et "}
          <code>{"serde"}</code>
          {" (fonctionnalité "}
          <code>{"derive"}</code>
          {")."}
        </p>
        <div className={"api-code-heading"}>
          <h3>{"Go"}</h3>
          <button type={"button"} className={"api-copy"} data-copy={"api-go"}>
            {"Copier"}
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
            {"Clients exécutables TypeScript, Rust et Go"}
          </a>
          {" · "}
          <a href={"https://github.com/bdlglua/bdl-pool/tree/main/api/tools"}>
            {"Monitoring, bot Discord et exporteur Prometheus"}
          </a>
        </p>
      </section>
      {"\n"}
      <section aria-labelledby={"api-format-title"}>
        <h2 id={"api-format-title"}>{"Format des données"}</h2>
        <dl className={"api-facts"}>
          <dt>{"Présence"}</dt>
          <dd>
            {
              "Relevé des connexions toutes les 15 secondes. available=false indique que leur état n’est pas confirmé."
            }
          </dd>
          <dt>{"Appareil"}</dt>
          <dd>
            {
              "Modèle annoncé dans le user-agent Stratum, sinon nom du logiciel. Un modèle absent reste null."
            }
          </dd>
          <dt>{"Actualisation"}</dt>
          <dd>
            {
              "Statistiques globales toutes les cinq minutes. Les données des workers suivent CKPool."
            }
          </dd>
          <dt>{"Dates"}</dt>
          <dd>
            {
              "updatedAt indique la date de la mesure. stale signale une mesure ancienne."
            }
          </dd>
          <dt>{"Hashrate"}</dt>
          <dd>
            {
              "Toutes les valeurs hashrateHps sont en H/s. Une valeur inconnue reste null."
            }
          </dd>
          <dt>{"Adresses"}</dt>
          <dd>
            {
              "Utilisez une adresse Bitcoin mainnet sans suffixe de worker. CKPool crée ses statistiques après sa première share enregistrée."
            }
          </dd>
          <dt>{"Workers"}</dt>
          <dd>
            {
              "limit vaut 50 par défaut, jusqu’à 200. offset permet de parcourir la liste. Le nom complet du worker évite les ambiguïtés."
            }
          </dd>
          <dt>{"Disponibilité"}</dt>
          <dd>
            {
              "readyAtLastCheck concerne le dernier contrôle Stratum V1. Il ne confirme pas une connexion V2."
            }
          </dd>
          <dt>{"Accès"}</dt>
          <dd>
            {
              "Lecture seule, sans authentification. GET, HEAD et OPTIONS ; CORS public. Aucun répertoire des adresses n’est exposé."
            }
          </dd>
        </dl>
      </section>
      {"\n"}
      <section aria-labelledby={"api-errors-title"}>
        <h2 id={"api-errors-title"}>{"Codes HTTP"}</h2>
        <dl className={"api-facts api-errors"}>
          <dt>
            <code>{"200"}</code>
          </dt>
          <dd>{"Données disponibles, parfois anciennes : vérifier stale."}</dd>
          <dt>
            <code>{"304"}</code>
          </dt>
          <dd>{"Réponse inchangée pour cet ETag."}</dd>
          <dt>
            <code>{"400"}</code>
          </dt>
          <dd>{"Adresse ou paramètres invalides."}</dd>
          <dt>
            <code>{"404"}</code>
          </dt>
          <dd>{"Endpoint, adresse ou worker absent."}</dd>
          <dt>
            <code>{"405"}</code>
          </dt>
          <dd>{"Méthode d’écriture refusée."}</dd>
          <dt>
            <code>{"503"}</code>
          </dt>
          <dd>{"Source de statistiques indisponible."}</dd>
        </dl>
      </section>
      {"\n"}
    </PoolPage>
  );
}
