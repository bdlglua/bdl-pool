/* BDL Pool — JavaScript natif, sans bibliothèque ni service de suivi. */
(() => {
  "use strict";
  const english = document.documentElement.lang === "en";
  const translations = {"Aller au contenu": "Skip to content", "BDL Pool, accueil": "BDL Pool, home", "Ouvrir le menu": "Open menu", "Fermer le menu": "Close menu", "Navigation principale": "Main navigation", "Vue d’ensemble": "Overview", "Connexion": "Connection", "Le solo mining": "Solo mining", "Profil GitHub de bdlglua (nouvel onglet)": "bdlglua on GitHub (new tab)", "Mon pool Bitcoin solo, hébergé sur une VM Freebox. Retrouvez ici l’état du nœud et les réglages pour connecter un mineur sur le réseau local.": "My solo Bitcoin pool, running on a Freebox VM. Check the node status and find the settings to connect a miner on the local network.", "Configurer mon mineur": "Set up a miner", "État du pool": "Pool status", "Sans inscription": "No registration", "SUIVI": "STATUS", "Relevé manuel": "Manual snapshot", "Actualiser le suivi": "Refresh status", "Chargement du dernier relevé…": "Loading latest snapshot…", "Puissance du pool": "Pool hashrate", "Hashrate mesuré": "Measured hashrate", "Mineurs actifs": "Active miners", "Workers connectés": "Connected workers", "Meilleure share": "Best share", "Difficulté atteinte": "Share difficulty", "Blocs trouvés": "Blocks found", "Blocs signalés par le pool": "Blocks reported by the pool", "Activité du pool": "Pool activity", "Pas encore de mesures": "No measurements yet", "Les mesures apparaîtront lorsque le suivi du pool sera connecté.": "Measurements will appear once monitoring is connected.", "Évolution du hashrate mesuré": "Measured hashrate over time", "Aucune mesure reçue": "No measurements received", "Données non disponibles": "No data available", "Le nœud Bitcoin": "Bitcoin node", "Synchronisation initiale": "Initial sync", "Progression estimée de la vérification": "Estimated verification progress", "Blocs validés": "Validated blocks", "En-têtes connus": "Known headers", "Le pool démarrera après la synchronisation complète du nœud.": "The pool will start after the node finishes syncing.", "Connecter un mineur": "Connect a miner", "Bitaxe et mineurs ASIC compatibles": "Compatible Bitaxe and ASIC miners", "Serveur Stratum": "Stratum server", "Type de connexion": "Connection type", "Réseau local": "Local network", "SERVEUR STRATUM": "STRATUM SERVER", "Copier l’adresse du serveur": "Copy server address", "Cette adresse est accessible uniquement sur le réseau local de la Freebox.": "This address is only accessible on the Freebox local network.", "Pool en préparation. Gardez votre pool actuel pendant la synchronisation.": "Pool starting up. Keep using your current pool while the node syncs.", "Votre adresse de réception suffit. Aucune clé privée.": "Use your receiving address. No private key needed.", "Préparer ma configuration": "Prepare miner settings", "Adresse de réception Bitcoin": "Bitcoin receiving address", "Votre adresse BTC : bc1…, 1… ou 3…": "Your BTC address: bc1…, 1… or 3…", "Copiez l’adresse BTC depuis votre portefeuille. Elle reste dans votre navigateur.": "Copy your BTC address from your wallet. It stays in your browser.", "Nom du mineur": "Worker name", "facultatif": "optional", "Générer mes réglages": "Generate settings", "À renseigner dans votre mineur": "Enter these settings in your miner", "Copier tous les réglages": "Copy all settings", "Serveur": "Server", "Utilisateur": "Username", "Mot de passe": "Password", "Ces réglages ne sont pas envoyés au mineur. Appliquez-les une fois le pool disponible.": "These settings are not sent to your miner. Apply them once the pool is available.", "FONCTIONNEMENT": "HOW IT WORKS", "Miner en solo": "Solo mining", "Vos mineurs participent à la recherche d’un bloc Bitcoin. Si l’un d’eux trouve un bloc valide et accepté par le réseau, sa récompense est attribuée à l’adresse utilisée par ce mineur.": "Your miners work to find a Bitcoin block. If one finds a valid block accepted by the network, the reward is assigned to that miner’s address.", "Comprendre avant de se lancer": "Read the FAQ", "Adresse BTC comme identifiant": "Your BTC address is your username", "L’adresse BTC sert d’identifiant. Aucun solde hébergé sur ce site, aucun compte à créer.": "Your BTC address is your username. This site holds no balance and requires no account.", "Frais du pool": "Pool fees", "Les frais dépendent de la configuration de CKPool sur le serveur. Ils ne sont pas vérifiés par ce site.": "Fees depend on the server’s CKPool configuration. This site does not verify them.", "Pas de paiement par share": "No payment per share", "Une share acceptée ne garantit pas un bloc. Le solo mining ne génère pas de revenu régulier.": "An accepted share does not guarantee a block. Solo mining does not provide regular income.", "AIDE": "HELP", "Questions fréquentes": "Frequently asked questions", "Connexion, disponibilité et récompenses.": "Connection, availability and rewards.", "Le pool est-il déjà disponible ?": "Is the pool available yet?", "Le dernier relevé manuel indique une synchronisation initiale en cours. L’accès Stratum est prévu après cette étape. Tant qu’un suivi en direct n’est pas connecté, ce site ne confirme pas la disponibilité actuelle du serveur.": "The latest manual snapshot shows initial sync in progress. Stratum access is planned afterwards. Until live monitoring is connected, this site cannot confirm current server availability.", "Puis-je me connecter depuis Internet ?": "Can I connect over the Internet?", "L’adresse locale affichée fonctionne uniquement sur le réseau de la Freebox. Un accès extérieur demande une adresse publique et une ouverture du port Stratum sur le serveur. Cet accès public n’est pas encore configuré sur ce site.": "The local address only works on the Freebox network. External access requires a public address and an accessible Stratum port. Public access is not configured on this site yet.", "Est-ce que GitHub héberge le mineur ?": "Does GitHub host the mining server?", "GitHub Pages héberge l’interface web. Le nœud Bitcoin et CKPool restent dans la VM de la Freebox. Un site disponible ne signifie donc pas automatiquement que le serveur de minage est disponible.": "GitHub Pages hosts the web interface. Bitcoin Core and CKPool run on the Freebox VM. A working website does not mean the mining server is available.", "Est-ce que je gagne à chaque share ?": "Do I earn a reward for each share?", "Non. Les shares servent à vérifier et mesurer le travail. En mode solo, une récompense dépend de la découverte d’un bloc valide et de son acceptation par le réseau. Les récompenses de bloc sont soumises à la maturité Bitcoin de 100 blocs.": "No. Shares measure and verify work. Solo rewards depend on finding a valid block accepted by the network. Block rewards have a 100-block maturity period.", "Faut-il fournir une clé privée ou une phrase de récupération ?": "Do I need a private key or recovery phrase?", "Jamais. Utilisez uniquement une adresse de réception du réseau Bitcoin BTC. Le configurateur traite cette adresse dans votre navigateur, sans l’enregistrer ni la transmettre à un service.": "Never. Only use a Bitcoin BTC receiving address. The form processes it in your browser without saving it or sending it to a service.", "CKPool sur VM Freebox · par bdlglua": "CKPool on a Freebox VM · by bdlglua", "Par bdlglua": "By bdlglua", "Non configuré": "Not configured", "Utilisez cette adresse depuis un réseau extérieur, après vérification de son ouverture.": "Use this address from an external network once access has been verified.", "L’accès public n’est pas encore configuré. Utilisez le réseau local du propriétaire du pool.": "Public access is not configured yet. Use the pool owner’s local network.", "Le suivi indique un pool disponible. Vérifiez les shares acceptées après connexion.": "Monitoring reports the pool as available. Check for accepted shares after connecting.", "Disponibilité non confirmée en direct. Gardez votre pool actuel pendant la préparation.": "Availability is not confirmed live. Keep using your current pool during setup.", "Copié dans le presse-papiers.": "Copied to clipboard.", "Copie indisponible : sélectionnez le texte affiché pour le copier.": "Copy unavailable: select the displayed text to copy it.", "date non renseignée": "date not provided", " mesures · heure de Paris": " samples · Paris time", "Suivi indisponible": "Monitoring unavailable", "Le suivi est indisponible ou trop ancien. Les dernières données conservées ne confirment pas la disponibilité du pool.": "Monitoring is unavailable or outdated. Stored data does not confirm pool availability.", "Dernier relevé manuel : ": "Latest manual snapshot: ", ". Ces informations ne sont pas actualisées en direct ; les mesures inconnues restent vides.": ". This is not live data; unknown measurements are left blank.", "Pool disponible": "Pool available", "Pool en préparation": "Pool starting up", "Suivi connecté · relevé du ": "Monitoring connected · snapshot from ", " (heure de Paris).": " (Paris time).", "Dernier état connu": "Last known status", "Synchronisé · relevé": "Synced · snapshot", "Synchronisé": "Synced", "État actuel non confirmé. Vérifiez le serveur avant de connecter un mineur.": "Current status unconfirmed. Check the server before connecting a miner.", "État du dernier relevé. Le serveur prévoit un démarrage du pool après synchronisation.": "Last recorded status. The server is set to start the pool after syncing.", "Vérification initiale terminée. La disponibilité Stratum dépend aussi de CKPool.": "Initial verification complete. Stratum availability also depends on CKPool.", "Le pool attend la fin de la synchronisation complète du nœud.": "The pool is waiting for the node to finish syncing.", "Dernières données conservées": "Last stored data", "Dernière mesure manuelle": "Last manual measurement", "Mesures reçues": "Measurements received", "Le suivi est actuellement indisponible ou trop ancien. Vérifiez le serveur avant de connecter un mineur.": "Monitoring is unavailable or outdated. Check the server before connecting a miner.", "Le dernier relevé manuel indique un pool en préparation. Ce site ne confirme pas sa disponibilité actuelle tant que le suivi en direct n’est pas connecté.": "The latest manual snapshot shows the pool starting up. Current availability cannot be confirmed until live monitoring is connected.", "Le suivi indique un nœud synchronisé et un pool disponible. Après connexion, vérifiez la réception de travail et les shares acceptées dans votre mineur.": "Monitoring reports a synced node and an available pool. After connecting, check for jobs and accepted shares on your miner.", "Le suivi indique un pool en préparation. Gardez votre mineur sur son pool actuel tant que le serveur n’est pas prêt.": "Monitoring reports that the pool is starting up. Keep using your current pool until the server is ready.", "Relevé manuel chargé. Le suivi en direct n’est pas encore connecté.": "Manual snapshot loaded. Live monitoring is not connected yet.", "Relevé trop ancien : disponibilité non confirmée.": "Snapshot outdated: availability unconfirmed.", "Suivi actualisé.": "Status refreshed.", "État inconnu": "Unknown status", "Le relevé n’a pas pu être chargé. La disponibilité actuelle du pool n’est pas confirmée.": "Could not load the snapshot. Current pool availability is unconfirmed.", "Aucune information de suivi disponible.": "No monitoring data available.", "Impossible de charger le suivi. Réessayez plus tard.": "Could not load monitoring data. Try again later.", "L’adresse de connexion n’est pas encore configurée.": "The connection address is not configured yet.", "Vérifiez le format de votre adresse Bitcoin BTC : bc1…, 1… ou 3….": "Check your Bitcoin BTC address format: bc1…, 1… or 3….", "Nom du mineur : 32 caractères maximum, lettres, chiffres, tiret ou underscore.": "Worker name: up to 32 letters, digits, hyphens or underscores.", "Serveur : ": "Server: ", "Utilisateur : ": "Username: ", "Mot de passe : x": "Password: x"};
  Object.assign(translations, {"Mise à jour automatique · ": "Automatic snapshot · ", " · toutes les 5 min, avec un délai possible de cache GitHub.": " · every 5 min; GitHub caching may delay display.", " · dernier relevé : ": " · last snapshot: ", "Nœud inaccessible": "Node unavailable", "Disponibilité à vérifier": "Availability unconfirmed", "Stratum répond, mais aucun travail n’a été confirmé par le contrôle automatique. Vérifiez les jobs et les shares dans votre mineur.": "Stratum responds, but the automatic check has not confirmed a mining job. Check jobs and shares on your miner.", "Pool disponible au dernier contrôle. Vérifiez les shares acceptées après connexion.": "Pool available at the last check. Verify accepted shares after connecting.", "Le relevé automatique indique un nœud synchronisé et un travail Stratum reçu lors du dernier contrôle. Vérifiez les shares acceptées sur votre mineur.": "The automatic snapshot shows a synced node and a Stratum job received at the last check. Verify accepted shares on your miner.", "Bitcoin Core ne répond pas au collecteur. Les données du nœud sont indisponibles.": "Bitcoin Core is not responding to the collector. Node data is unavailable.", "Dernier contrôle automatique : ": "Last automatic check: ", "Le pool attend la synchronisation du nœud.": "The pool is waiting for the node to sync.", "Actualisation automatique toutes les cinq minutes.": "Automatic updates every five minutes."});
  const t = (text) => english ? (translations[text] || text) : text;
  const locale = english ? "en-GB" : "fr-FR";
  const config = window.BDL_POOL_CONFIG || {};
  const $ = (id) => document.getElementById(id);
  const number = new Intl.NumberFormat(locale);
  const compact = new Intl.NumberFormat(locale, { notation: "compact", maximumFractionDigits: 2 });
  let network = "local";
  let settings = null;
  let toastTimer;
  let loading = false;
  let statusData = null;
  let statusFailed = false;
  let statusStale = false;
  let latestPoll = 0;

  const finite = (value) => typeof value === "number" && Number.isFinite(value) && value >= 0;
  function toast(message) {
    clearTimeout(toastTimer);
    $("toast").textContent = message;
    $("toast").hidden = false;
    toastTimer = setTimeout(() => { $("toast").hidden = true; }, 4200);
  }
  async function copy(text) {
    try {
      if (!navigator.clipboard || !window.isSecureContext) throw new Error("clipboard unavailable");
      await navigator.clipboard.writeText(text);
      toast(t("Copié dans le presse-papiers."));
    } catch {
      toast(t("Copie indisponible : sélectionnez le texte affiché pour le copier."));
    }
  }
  function hashrate(value) {
    if (!finite(value)) return "—";
    const units = ["H/s", "kH/s", "MH/s", "GH/s", "TH/s", "PH/s", "EH/s"];
    let index = 0;
    while (value >= 1000 && index < units.length - 1) { value /= 1000; index++; }
    return new Intl.NumberFormat(locale, { maximumFractionDigits: 2 }).format(value) + " " + units[index];
  }
  function dateLabel(value) {
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return t("date non renseignée");
    return new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeZone: "Europe/Paris" }).format(date);
  }
  function badge(id, text, style) {
    const element = $(id);
    element.textContent = text;
    element.className = "badge " + style;
  }
  function currentlyReady() {
    return !statusFailed && !statusStale && ["live", "snapshot"].includes(statusData?.source) && statusData?.pool?.ready === true && statusData?.node?.initialBlockDownload === false;
  }
  function currentHost() {
    return network === "public" ? String(config.publicHost || "").trim() : String(config.localHost || "").trim();
  }
  function renderEndpoint() {
    const host = currentHost();
    const publicUnavailable = network === "public" && !host;
    $("endpoint-host").textContent = host || t("Non configuré");
    $("endpoint-port").textContent = config.stratumPort || 3333;
    $("copy-host").disabled = !host;
    $("miner-form").querySelector("button[type=submit]").disabled = publicUnavailable;
    $("network-note").textContent = network === "local"
      ? t("Cette adresse est accessible uniquement sur le réseau local de la Freebox.")
      : host ? t("Utilisez cette adresse depuis un réseau extérieur, après vérification de son ouverture.")
      : t("L’accès public n’est pas encore configuré. Utilisez le réseau local du propriétaire du pool.");
    $("connection-availability").lastElementChild.textContent = currentlyReady()
      ? t("Pool disponible au dernier contrôle. Vérifiez les shares acceptées après connexion.")
      : t("Disponibilité non confirmée en direct. Gardez votre pool actuel pendant la préparation.");
    document.querySelectorAll("[data-network]").forEach((button) => {
      const selected = button.dataset.network === network;
      button.classList.toggle("selected", selected);
      button.setAttribute("aria-pressed", String(selected));
    });
  }
  function clearResult() {
    settings = null;
    $("miner-result").hidden = true;
    $("form-error").hidden = true;
    $("btc-address").removeAttribute("aria-invalid");
    $("worker-name").removeAttribute("aria-invalid");
  }
  function svgElement(tag, attributes) {
    const element = document.createElementNS("http://www.w3.org/2000/svg", tag);
    for (const [key, value] of Object.entries(attributes)) element.setAttribute(key, String(value));
    return element;
  }
  function renderChart(history) {
    const svg = $("hash-chart");
    svg.replaceChildren();
    const points = Array.isArray(history) ? history.filter((entry) => entry && finite(entry.hashrateHps) && Number.isFinite(Date.parse(entry.time))).sort((a, b) => Date.parse(a.time) - Date.parse(b.time)).slice(-120) : [];
    if (points.length < 2) {
      svg.setAttribute("hidden", "");
      $("chart-empty").hidden = false;
      $("sample-count").textContent = t("Données non disponibles");
      return;
    }
    svg.removeAttribute("hidden");
    $("chart-empty").hidden = true;
    const start = Date.parse(points[0].time), end = Date.parse(points.at(-1).time);
    const peak = Math.max(...points.map((p) => p.hashrateHps), 1);
    const xy = points.map((p) => [48 + (Date.parse(p.time) - start) / (end - start || 1) * 582, 142 - p.hashrateHps / peak * 123]);
    for (const [ratio, label] of [[0, hashrate(peak)], [0.5, hashrate(peak / 2)], [1, "0"]]) {
      const y = 19 + ratio * 123;
      svg.appendChild(svgElement("line", { x1: 48, y1: y, x2: 630, y2: y, stroke: "#ffffff0c" }));
      const text = svgElement("text", { x: 0, y: y + 3, class: "chart-label" });
      text.textContent = label;
      svg.appendChild(text);
    }
    svg.appendChild(svgElement("polyline", { points: xy.map((p) => p.join(",")).join(" "), fill: "none", stroke: "#f6a046", "stroke-width": 2, "stroke-linejoin": "round" }));
    const formatTime = (value) => new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit", timeZone: "Europe/Paris" }).format(new Date(value));
    for (const [x, anchor, time] of [[48, "start", points[0].time], [630, "end", points.at(-1).time]]) {
      const text = svgElement("text", { x, y: 165, "text-anchor": anchor, class: "chart-label" });
      text.textContent = formatTime(time);
      svg.appendChild(text);
    }
    $("sample-count").textContent = points.length + t(" mesures · heure de Paris");
  }
  function timestampLabel(value) {
    const date = new Date(value);
    return Number.isFinite(date.getTime())
      ? new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short", timeZone: "Europe/Paris" }).format(date) + t(" (heure de Paris).")
      : t("date non renseignée");
  }
  function renderStatus() {
    const data = statusData;
    if (!data) return;
    const manual = data.source === "manual";
    const snapshot = data.source === "snapshot";
    const node = data.node || {};
    const pool = data.pool || {};
    const progress = finite(node.verificationProgress) ? Math.min(1, node.verificationProgress) * 100 : null;
    statusStale = !manual && (!Number.isFinite(Date.parse(data.updatedAt)) || Date.now() - Date.parse(data.updatedAt) > (snapshot ? 900000 : 180000) || Date.parse(data.updatedAt) > Date.now() + 60000);
    const unreliable = statusFailed || statusStale;
    const nodeUnknown = typeof node.initialBlockDownload !== "boolean" || data.checks?.bitcoin === "unavailable";
    if (unreliable) {
      badge("pool-badge", t("Suivi indisponible"), "neutral");
      $("data-notice").textContent = t("Le suivi est indisponible ou trop ancien. Les dernières données conservées ne confirment pas la disponibilité du pool.") + t(" · dernier relevé : ") + timestampLabel(data.updatedAt);
    } else if (manual) {
      badge("pool-badge", t("Relevé manuel"), "amber");
      $("data-notice").textContent = t("Dernier relevé manuel : ") + dateLabel(data.updatedAt) + t(". Ces informations ne sont pas actualisées en direct ; les mesures inconnues restent vides.");
    } else {
      const label = currentlyReady() ? t("Pool disponible") : nodeUnknown ? t("Nœud inaccessible") : node.initialBlockDownload === true ? t("Synchronisation initiale") : t("Disponibilité à vérifier");
      badge("pool-badge", label, currentlyReady() ? "green" : "amber");
      $("data-notice").textContent = snapshot
        ? t("Mise à jour automatique · ") + timestampLabel(data.updatedAt) + t(" · toutes les 5 min, avec un délai possible de cache GitHub.")
        : t("Suivi connecté · relevé du ") + timestampLabel(data.updatedAt);
    }
    $("metric-hashrate").textContent = hashrate(pool.hashrateHps);
    $("metric-workers").textContent = finite(pool.workers) ? number.format(pool.workers) : "—";
    $("metric-best").textContent = finite(pool.bestShare) ? compact.format(pool.bestShare) : "—";
    $("metric-blocks").textContent = finite(pool.blocksFound) ? number.format(pool.blocksFound) : "—";
    $("node-blocks").textContent = finite(node.blocks) ? number.format(node.blocks) : "—";
    $("node-headers").textContent = finite(node.headers) ? number.format(node.headers) : "—";
    $("sync-value").replaceChildren(document.createTextNode(progress === null ? "—" : new Intl.NumberFormat(locale, { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(progress)));
    const percent = document.createElement("span"); percent.textContent = " %"; $("sync-value").appendChild(percent);
    $("sync-fill").style.width = (progress || 0) + "%";
    $("sync-progress").setAttribute("aria-valuenow", String(progress || 0));
    const nodeSynced = node.initialBlockDownload === false;
    badge("node-badge", nodeUnknown ? t("État inconnu") : unreliable ? t("Dernier état connu") : nodeSynced ? (manual ? t("Synchronisé · relevé") : t("Synchronisé")) : t("Synchronisation initiale"), unreliable || nodeUnknown ? "neutral" : nodeSynced ? "green" : "amber");
    $("node-note").textContent = nodeUnknown ? t("Bitcoin Core ne répond pas au collecteur. Les données du nœud sont indisponibles.") : unreliable ? t("État actuel non confirmé. Vérifiez le serveur avant de connecter un mineur.") : manual ? t("État du dernier relevé. Le serveur prévoit un démarrage du pool après synchronisation.") : nodeSynced ? t("Vérification initiale terminée. La disponibilité Stratum dépend aussi de CKPool.") : t("Le pool attend la fin de la synchronisation complète du nœud.");
    $("activity-state").textContent = unreliable ? t("Dernières données conservées") : finite(pool.hashrateHps) ? (manual ? t("Dernière mesure manuelle") : t("Mesures reçues")) : t("Aucune mesure reçue");
    $("faq-availability").textContent = unreliable ? t("Le suivi est actuellement indisponible ou trop ancien. Vérifiez le serveur avant de connecter un mineur.") : manual ? t("Le dernier relevé manuel indique un pool en préparation. Ce site ne confirme pas sa disponibilité actuelle tant que le suivi en direct n’est pas connecté.") : currentlyReady() ? t("Le relevé automatique indique un nœud synchronisé et un travail Stratum reçu lors du dernier contrôle. Vérifiez les shares acceptées sur votre mineur.") : t("Le suivi indique un pool en préparation. Gardez votre mineur sur son pool actuel tant que le serveur n’est pas prêt.");
    if (!unreliable && !manual && data.checks?.stratum === "subscribed_no_job") {
      $("faq-availability").textContent = t("Stratum répond, mais aucun travail n’a été confirmé par le contrôle automatique. Vérifiez les jobs et les shares dans votre mineur.");
    }
    renderChart(data.history);
    renderEndpoint();
  }
  async function loadStatus(manualClick = false) {
    if (loading) return;
    loading = true;
    $("refresh-status").disabled = true;
    try {
      const url = new URL(config.statusUrl || "./data/status.json", document.baseURI);
      if (url.origin !== location.origin && url.protocol !== "https:") throw new Error("HTTPS required");
      const response = await fetch(url, { cache: "no-store", credentials: "omit", signal: AbortSignal.timeout(10000) });
      if (!response.ok) throw new Error("Status unavailable");
      const data = await response.json();
      if (data.schemaVersion !== 1 || !data.node || !data.pool || !["manual", "live", "snapshot"].includes(data.source)) throw new Error("Invalid status schema");
      statusData = data;
      statusFailed = false;
      latestPoll = Date.now();
      renderStatus();
      if (manualClick) toast(data.source === "manual" ? t("Relevé manuel chargé. Le suivi en direct n’est pas encore connecté.") : statusStale ? t("Relevé trop ancien : disponibilité non confirmée.") : t("Suivi actualisé."));
    } catch {
      statusFailed = true;
      if (statusData) renderStatus();
      else {
        badge("pool-badge", t("Suivi indisponible"), "neutral");
        badge("node-badge", t("État inconnu"), "neutral");
        $("data-notice").textContent = t("Le relevé n’a pas pu être chargé. La disponibilité actuelle du pool n’est pas confirmée.");
        $("node-note").textContent = t("Aucune information de suivi disponible.");
        renderEndpoint();
      }
      if (manualClick) toast(t("Impossible de charger le suivi. Réessayez plus tard."));
    } finally {
      loading = false;
      $("refresh-status").disabled = false;
    }
  }
  $("refresh-status").addEventListener("click", () => loadStatus(true));
  $("copy-host").addEventListener("click", () => { const host = currentHost(); if (host) copy(host); });
  document.querySelectorAll("[data-network]").forEach((button) => button.addEventListener("click", () => { network = button.dataset.network; clearResult(); renderEndpoint(); }));
  for (const id of ["btc-address", "worker-name"]) $(id).addEventListener("input", clearResult);
  $("miner-form").addEventListener("submit", (event) => {
    event.preventDefault();
    clearResult();
    const host = currentHost();
    const address = $("btc-address").value.trim();
    const worker = $("worker-name").value.trim();
    // Contrôle du format uniquement ; ce formulaire ne valide pas la somme de contrôle.
    const addressFormat = /^(?:[13][1-9A-HJ-NP-Za-km-z]{25,34}|bc1[ac-hj-np-z02-9]{11,87})$/;
    let error = "", focus = "btc-address";
    if (!host) error = t("L’adresse de connexion n’est pas encore configurée.");
    else if (!addressFormat.test(address)) error = t("Vérifiez le format de votre adresse Bitcoin BTC : bc1…, 1… ou 3….");
    else if (worker && !/^[a-zA-Z0-9_-]{1,32}$/.test(worker)) { error = t("Nom du mineur : 32 caractères maximum, lettres, chiffres, tiret ou underscore."); focus = "worker-name"; }
    if (error) {
      $("form-error").textContent = error;
      $("form-error").hidden = false;
      $(focus).setAttribute("aria-invalid", "true");
      $(focus).focus();
      return;
    }
    settings = { host, port: config.stratumPort || 3333, username: address + (worker ? "." + worker : ""), password: "x" };
    $("result-host").textContent = settings.host;
    $("result-port").textContent = settings.port;
    $("result-user").textContent = settings.username;
    $("miner-result").hidden = false;
    $("miner-result").scrollIntoView({ behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth", block: "nearest" });
  });
  $("copy-settings").addEventListener("click", () => {
    if (!settings) return;
    copy(t("Serveur : ") + settings.host + "\nPort : " + settings.port + (english ? "\nUsername: " : "\nUtilisateur : ") + settings.username + (english ? "\nPassword: x\nStratum URL: stratum+tcp://" : "\nMot de passe : x\nURL Stratum : stratum+tcp://") + settings.host + ":" + settings.port);
  });
  document.querySelector(".menu-toggle").addEventListener("click", (event) => {
    const button = event.currentTarget;
    const open = button.getAttribute("aria-expanded") !== "true";
    button.setAttribute("aria-expanded", String(open));
    button.setAttribute("aria-label", open ? t("Fermer le menu") : t("Ouvrir le menu"));
    $("main-nav").classList.toggle("open", open);
  });
  document.querySelectorAll(".main-nav a").forEach((link) => link.addEventListener("click", () => {
    $("main-nav").classList.remove("open");
    document.querySelector(".menu-toggle").setAttribute("aria-expanded", "false");
    document.querySelector(".menu-toggle").setAttribute("aria-label", t("Ouvrir le menu"));
  }));
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      $("main-nav").classList.remove("open");
      document.querySelector(".menu-toggle").setAttribute("aria-expanded", "false");
      document.querySelector(".menu-toggle").setAttribute("aria-label", t("Ouvrir le menu"));
    }
  });
  if ("IntersectionObserver" in window) {
    const sections = ["dashboard", "connexion", "solo", "faq"].map($);
    const observer = new IntersectionObserver((entries) => {
      const visible = entries.filter((e) => e.isIntersecting).sort((a, b) => b.intersectionRatio - a.intersectionRatio)[0];
      if (!visible) return;
      document.querySelectorAll(".main-nav a").forEach((link) => link.classList.toggle("active", link.getAttribute("href") === "#" + visible.target.id));
    }, { rootMargin: "-10% 0px -45% 0px", threshold: [0, .2, .6] });
    sections.forEach((section) => observer.observe(section));
  }
  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "visible" && Date.now() - latestPoll > 60000) loadStatus();
  });
  setInterval(() => {
    if (document.visibilityState !== "visible") return;
    loadStatus();
    if (statusData) renderStatus();
  }, Math.max(15000, Number(config.pollIntervalMs) || 60000));
  renderEndpoint();
  loadStatus();
})();
