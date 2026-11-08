/* Configuration publique uniquement : aucune clé, aucun mot de passe RPC. */
window.BDL_POOL_CONFIG = Object.freeze({
  localHost: "192.168.1.2",
  stratumPort: 3333,
  publicPort: 3333, // Port Internet : peut différer du port local.
  publicHost: "bdl-pool.duckdns.org", // DNS vérifié vers l’adresse publique du pool.
  sv2: Object.freeze({
    enabled: false, // À activer après configuration et vérification du serveur.
    stratumPort: 3336,
    publicPort: 3336,
    authorityPublicKey: "" // Clé PUBLIQUE Noise uniquement ; jamais une clé privée.
  }),
  statusUrl: "https://raw.githubusercontent.com/bdlglua/bdl-pool/stats/status.json", // Relevés publics filtrés.
  pollIntervalMs: 60000
});
