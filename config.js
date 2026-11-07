/* Configuration publique uniquement : aucune clé, aucun mot de passe RPC. */
window.BDL_POOL_CONFIG = Object.freeze({
  localHost: "192.168.1.2",
  stratumPort: 3333,
  publicHost: "", // Exemple après ouverture effective : pool.votre-domaine.fr
  statusUrl: "https://raw.githubusercontent.com/bdlglua/bdl-pool/stats/status.json", // Relevés filtrés publiés par la VM.
  pollIntervalMs: 60000
});
