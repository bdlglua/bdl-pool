/* Paramètres publics uniquement : jamais de clé privée ni d’identifiants RPC. */
window.BDL_POOL_CONFIG = Object.freeze({
  publicPort: 39991,
  publicHost: "stratum.bdl-pool.com", // Adresse de minage, distincte du site web.
  sv2: Object.freeze({
    enabled: true, // Paramètres de connexion disponibles ; disponibilité à vérifier séparément.
    publicPort: 36809,
    authorityPublicKey: "9c9DhUCzDsSZaCgKJitKKcngq24FLrqfJ9kUQzhxLiiSvAGuhna" // Clé PUBLIQUE Noise uniquement.
  }),
  statusUrl: "https://raw.githubusercontent.com/bdlglua/bdl-pool/stats/status.json", // Relevés publics filtrés.
  apiBaseUrl: "https://api.bdl-pool.com:36018/api/v1",
  coins: Object.freeze({
    bch: Object.freeze({
      // The API supplies the real public BCH endpoint after server installation.
      publicHost: "",
      publicPort: null,
      sv2: Object.freeze({ enabled: false }),
      statusUrl: "https://raw.githubusercontent.com/bdlglua/bdl-pool/stats/status-bch.json",
      apiBaseUrl: "https://api.bdl-pool.com:36018/api/v1/coins/bch"
    })
  }),
  pollIntervalMs: 60000
});
