/* Paramètres publics uniquement : jamais de clé privée ni d’identifiants RPC. */
window.BDL_POOL_CONFIG = Object.freeze({
  publicPort: 39991,
  publicHost: "bdl-pool.duckdns.org", // Nom public dirigé vers le relais OpenPort.
  sv2: Object.freeze({
    enabled: true, // Paramètres de connexion disponibles ; disponibilité à vérifier séparément.
    publicPort: 36809,
    authorityPublicKey: "9c9DhUCzDsSZaCgKJitKKcngq24FLrqfJ9kUQzhxLiiSvAGuhna" // Clé PUBLIQUE Noise uniquement.
  }),
  statusUrl: "https://raw.githubusercontent.com/bdlglua/bdl-pool/stats/status.json", // Relevés publics filtrés.
  apiBaseUrl: "https://bdl-pool.duckdns.org:36018/api/v1",
  pollIntervalMs: 60000
});
