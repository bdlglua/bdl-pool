import { PoolPage } from "@/components/PoolPage";
import { pageMetadata } from "@/lib/pages";

export const metadata = pageMetadata("faq", "fr");

export default function Page() {
  return (
    <PoolPage lang="fr" page="faq">
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
              {"Le pool est-il disponible ?"}
              <span>{"+"}</span>
            </summary>
            <p id={"faq-availability"}>{"État du pool non confirmé."}</p>
          </details>
          <details>
            <summary>
              {"Puis-je me connecter depuis Internet ?"}
              <span>{"+"}</span>
            </summary>
            <p id="faq-connection">
              {
                "Oui. Utilisez stratum.bdl-pool.com : port 39991 pour Stratum V1 ou port 36809 pour Stratum V2 avec la clé publique : 9c9DhUCzDsSZaCgKJitKKcngq24FLrqfJ9kUQzhxLiiSvAGuhna."
              }
            </p>
          </details>
          <details>
            <summary>
              {"Une share rapporte-t-elle une récompense ?"}
              <span>{"+"}</span>
            </summary>
            <p>
              {
                "Non. Il faut trouver un bloc accepté par le réseau Bitcoin. Sa récompense est disponible après 100 blocs."
              }
            </p>
          </details>
        </div>
      </section>
      {"\n"}
    </PoolPage>
  );
}
