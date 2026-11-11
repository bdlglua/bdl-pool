import { PoolPage } from "@/components/PoolPage";
import { pageMetadata } from "@/lib/pages";

export const metadata = pageMetadata("faq", "en");

export default function Page() {
  return (
    <PoolPage lang="en" page="faq">
      <section
        className={"faq-section wrap category-page"}
        id={"faq"}
        aria-labelledby={"faq-title"}
      >
        <div className={"faq-intro"}>
          <h1 id={"faq-title"}>{"FAQ"}</h1>
        </div>
        <div className={"faq-list"}>
          <details>
            <summary>
              {"Is the pool available?"}
              <span>{"+"}</span>
            </summary>
            <p id={"faq-availability"}>{"Pool status unconfirmed."}</p>
          </details>
          <details>
            <summary>
              {"Can I connect over the Internet?"}
              <span>{"+"}</span>
            </summary>
            <p id="faq-connection">
              {
                "Yes. Use stratum.bdl-pool.com: port 39991 for V1 or 36809 for V2."
              }
            </p>
          </details>
          <details>
            <summary>
              {"Does each share earn a reward?"}
              <span>{"+"}</span>
            </summary>
            <p>
              {
                "No. A reward requires a block accepted by the Bitcoin network. It matures after 100 blocks."
              }
            </p>
          </details>
        </div>
      </section>
      {"\n"}
    </PoolPage>
  );
}
