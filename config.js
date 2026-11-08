/* Paramètres publics uniquement : jamais de clé privée ni d’identifiants RPC. */
window.BDL_POOL_CONFIG = Object.freeze({
  localHost: "192.168.1.2",
  stratumPort: 3333,
  publicPort: 3333, // Port Internet : peut différer du port local.
  publicHost: "bdl-pool.duckdns.org", // DNS vérifié vers l’adresse publique du pool.
  sv2: Object.freeze({
    enabled: true, // Paramètres de connexion disponibles ; disponibilité à vérifier séparément.
    stratumPort: 3336,
    publicPort: 3336,
    authorityPublicKey: "9c9DhUCzDsSZaCgKJitKKcngq24FLrqfJ9kUQzhxLiiSvAGuhna" // Clé PUBLIQUE Noise uniquement.
  }),
  statusUrl: "https://raw.githubusercontent.com/bdlglua/bdl-pool/stats/status.json", // Relevés publics filtrés.
  pollIntervalMs: 60000
});
