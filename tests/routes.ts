// Independent route/metadata expectations for the public website.

export type Language = "fr" | "en";
export const pages = {
  home: {
    slugs: ["", ""],
    labels: ["Accueil", "Home"],
    descriptions: [
      "Pool Bitcoin solo indépendant, connexions Stratum V1 et V2 et état du nœud Bitcoin.",
      "Independent Bitcoin solo pool, Stratum V1/V2 connections and Bitcoin node status.",
    ],
  },
  statistics: {
    slugs: ["statistiques", "statistics"],
    labels: ["Statistiques", "Statistics"],
    descriptions: [
      "Hashrate du pool, workers, meilleures shares et synchronisation du nœud Bitcoin.",
      "Pool hashrate, workers, best shares and Bitcoin node synchronisation.",
    ],
  },
  miners: {
    slugs: ["mineurs", "miners"],
    labels: ["Mineurs", "Miners"],
    descriptions: [
      "Recherchez les statistiques de votre adresse Bitcoin et consultez les mineurs en ligne.",
      "Look up your Bitcoin address statistics and view online miners.",
    ],
  },
  connect: {
    slugs: ["connexion", "connect"],
    labels: ["Connexion", "Connection"],
    descriptions: [
      "Adresse, ports et clé publique Stratum V2 pour configurer votre mineur Bitcoin.",
      "Address, ports and Stratum V2 public key to configure your Bitcoin miner.",
    ],
  },
  faq: {
    slugs: ["faq", "faq"],
    labels: ["FAQ", "FAQ"],
    descriptions: [
      "Questions sur la connexion, la disponibilité et les récompenses du pool Bitcoin.",
      "Questions about connecting, pool availability and Bitcoin block rewards.",
    ],
  },
  api: {
    slugs: ["api", "api"],
    labels: ["API", "API"],
    descriptions: [
      "Statistiques du pool, des adresses Bitcoin et des workers via une API publique.",
      "Pool, Bitcoin address and worker statistics through the public API.",
    ],
  },
} as const;
export type PageName = keyof typeof pages;
export const pageNames = Object.keys(pages) as PageName[];
export const origin = "https://bdl-pool.com";

export function route(page: PageName, lang: Language): string {
  const slug = pages[page].slugs[lang === "en" ? 1 : 0];
  return (lang === "en" ? "/en/" : "/") + (slug ? `${slug}/` : "");
}
