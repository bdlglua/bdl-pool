import { PoolPage } from "@/components/PoolPage";
import { pageMetadata } from "@/lib/pages";

export const metadata = pageMetadata("connect", "en");

export default function Page() {
  return (
    <PoolPage lang="en" page="connect">
      <section
        className={"connection-section wrap category-page"}
        id={"connexion"}
        aria-labelledby={"connection-title"}
      >
        {"\n      "}
        <div className={"section-heading"}>
          <div>
            <h1 id={"connection-title"}>{"Connection"}</h1>
          </div>
        </div>
        {"\n      "}
        <div className={"connection-grid"}>
          {"\n        "}
          <div className={"panel endpoint-panel"}>
            <div className={"panel-heading"}>
              <h3>{"Stratum"}</h3>
            </div>
            <div
              className={"protocol-selector"}
              role={"group"}
              aria-label={"Stratum protocol"}
            >
              <button
                type={"button"}
                className={"selected"}
                data-protocol={"v1"}
                aria-pressed={"true"}
              >
                {"Stratum V1"}
              </button>
              <button
                type={"button"}
                data-protocol={"v2"}
                aria-pressed={"false"}
              >
                {"Stratum V2"}
              </button>
            </div>
            <p id={"protocol-note"} className={"protocol-note"} hidden></p>
            <div className={"endpoint-block"}>
              {"\n          "}
              <dl className={"endpoint-fields"}>
                {"\n            "}
                <div className={"endpoint-field"}>
                  <dt>{"Address"}</dt>
                  <dd>
                    <code id={"endpoint-host"}>{"stratum.bdl-pool.com"}</code>
                    <button
                      type={"button"}
                      id={"copy-host"}
                      className={"icon-button"}
                      aria-label={"Copy server address"}
                    >
                      <svg className={"icon"}>
                        <use href={"#i-copy"} />
                      </svg>
                    </button>
                  </dd>
                </div>
                {"\n            "}
                <div className={"endpoint-field"}>
                  <dt>{"Port"}</dt>
                  <dd>
                    <code id={"endpoint-port"}>{"39991"}</code>
                    <button
                      type={"button"}
                      id={"copy-port"}
                      className={"icon-button"}
                      aria-label={"Copy port"}
                    >
                      <svg className={"icon"}>
                        <use href={"#i-copy"} />
                      </svg>
                    </button>
                  </dd>
                </div>
                {"\n            "}
                <div
                  className={"endpoint-field endpoint-key"}
                  id={"endpoint-key-row"}
                  hidden
                >
                  <dt>{"Public encryption key"}</dt>
                  <dd>
                    <code id={"endpoint-key"}>{"—"}</code>
                    <button
                      type={"button"}
                      id={"copy-key"}
                      className={"icon-button"}
                      aria-label={"Copy public key"}
                    >
                      <svg className={"icon"}>
                        <use href={"#i-copy"} />
                      </svg>
                    </button>
                  </dd>
                </div>
                {"\n          "}
              </dl>
              {"\n          "}
              <button
                type={"button"}
                id={"copy-url"}
                className={"endpoint-copy-url"}
              >
                <svg className={"icon"}>
                  <use href={"#i-copy"} />
                </svg>
                <span>{"Copy full URL"}</span>
              </button>
              {"\n        "}
            </div>
            <div className={"availability-note"} id={"connection-availability"}>
              <span className={"dot"}></span>
              <span>{"Pool status unconfirmed."}</span>
            </div>
          </div>
          {"\n        "}
          <form className={"panel setup-form"} id={"miner-form"} noValidate>
            <div className={"panel-heading"}>
              <h3>{"Miner settings"}</h3>
            </div>
            <label htmlFor={"btc-address"}>
              <span>{"Bitcoin address"}</span>
            </label>
            <input
              id={"btc-address"}
              name={"address"}
              placeholder={"bc1…, 1… or 3…"}
              autoComplete={"off"}
              autoCapitalize={"off"}
              spellCheck={false}
              aria-describedby={"form-error"}
            />
            <label htmlFor={"worker-name"}>
              <span>{"Worker name"}</span>
              <span className={"optional"}>{"optional"}</span>
            </label>
            <input
              id={"worker-name"}
              name={"worker"}
              placeholder={"bitaxe-01"}
              maxLength={32}
              autoComplete={"off"}
              spellCheck={false}
            />
            <p
              className={"form-error"}
              id={"form-error"}
              role={"alert"}
              hidden
            ></p>
            <button type={"submit"} className={"button primary form-submit"}>
              <span>{"Show settings"}</span>
              <svg className={"icon"}>
                <use href={"#i-arrow"} />
              </svg>
            </button>
            <div id={"miner-result"} className={"miner-result"} hidden>
              <div className={"result-heading"}>
                <strong>{"Copy into your miner"}</strong>
                <button
                  type={"button"}
                  className={"icon-button"}
                  id={"copy-settings"}
                  aria-label={"Copy all settings"}
                >
                  <svg className={"icon"}>
                    <use href={"#i-copy"} />
                  </svg>
                </button>
              </div>
              <dl>
                <div>
                  <dt>{"Stratum URL"}</dt>
                  <dd id={"result-url"}></dd>
                </div>
                <div>
                  <dt>{"Address"}</dt>
                  <dd id={"result-host"}></dd>
                </div>
                <div>
                  <dt>{"Port"}</dt>
                  <dd id={"result-port"}></dd>
                </div>
                <div id={"result-key-row"} hidden>
                  <dt>{"Public encryption key"}</dt>
                  <dd id={"result-key"}></dd>
                </div>
                <div>
                  <dt>{"Username"}</dt>
                  <dd id={"result-user"}></dd>
                </div>
                <div>
                  <dt>{"Password"}</dt>
                  <dd id={"result-password"}>{"x"}</dd>
                </div>
              </dl>
            </div>
          </form>
          {"\n      "}
        </div>
        {"\n    "}
      </section>
      {"\n"}
    </PoolPage>
  );
}
