/* Configuration publique uniquement : aucune clé, aucun mot de passe RPC. */
window.BDL_POOL_CONFIG = Object.freeze({
  localHost: "192.168.1.2",
  stratumPort: 3333,
  publicPort: 3333, // Port Internet : peut différer du port local.
  publicHost: "", // Exemple après ouverture effective : pool.votre-domaine.fr
  statusUrl: "https://raw.githubusercontent.com/bdlglua/bdl-pool/stats/status.json", // Relevés publics filtrés.
  pollIntervalMs: 60000
});
