/* Paramètres publics uniquement : jamais de clé privée ni d’identifiants RPC. */
window.BDL_POOL_CONFIG = Object.freeze({
  publicPort: 3333,
  publicHost: "bdl-pool.duckdns.org", // DNS vérifié vers l’adresse publique du pool.
  sv2: Object.freeze({
    enabled: true, // Paramètres de connexion disponibles ; disponibilité à vérifier séparément.
    publicPort: 3336,
    authorityPublicKey: "9c9DhUCzDsSZaCgKJitKKcngq24FLrqfJ9kUQzhxLiiSvAGuhna" // Clé PUBLIQUE Noise uniquement.
  }),
  statusUrl: "https://raw.githubusercontent.com/bdlglua/bdl-pool/stats/status.json", // Relevés publics filtrés.
  pollIntervalMs: 60000
});
