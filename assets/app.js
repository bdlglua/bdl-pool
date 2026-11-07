/* BDL Pool — JavaScript natif, sans bibliothèque ni service de suivi. */
(() => {
  "use strict";
  const config = window.BDL_POOL_CONFIG || {};
  const $ = (id) => document.getElementById(id);
  const number = new Intl.NumberFormat("fr-FR");
  const compact = new Intl.NumberFormat("fr-FR", { notation: "compact", maximumFractionDigits: 2 });
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
      toast("Copié dans le presse-papiers.");
    } catch {
      toast("Copie indisponible : sélectionnez le texte affiché pour le copier.");
    }
  }
  function hashrate(value) {
    if (!finite(value)) return "—";
    const units = ["H/s", "kH/s", "MH/s", "GH/s", "TH/s", "PH/s", "EH/s"];
    let index = 0;
    while (value >= 1000 && index < units.length - 1) { value /= 1000; index++; }
    return new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 2 }).format(value) + " " + units[index];
  }
  function dateLabel(value) {
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return "date non renseignée";
    return new Intl.DateTimeFormat("fr-FR", { dateStyle: "medium", timeZone: "Europe/Paris" }).format(date);
  }
  function badge(id, text, style) {
    const element = $(id);
    element.textContent = text;
    element.className = "badge " + style;
  }
  function currentlyReady() {
    return !statusFailed && !statusStale && statusData?.source === "live" && statusData?.pool?.ready === true && statusData?.node?.initialBlockDownload === false;
  }
  function currentHost() {
    return network === "public" ? String(config.publicHost || "").trim() : String(config.localHost || "").trim();
  }
  function renderEndpoint() {
    const host = currentHost();
    const publicUnavailable = network === "public" && !host;
    $("endpoint-host").textContent = host || "Non configuré";
    $("endpoint-port").textContent = config.stratumPort || 3333;
    $("copy-host").disabled = !host;
    $("miner-form").querySelector("button[type=submit]").disabled = publicUnavailable;
    $("network-note").textContent = network === "local"
      ? "Cette adresse est accessible uniquement sur le réseau local de la Freebox."
      : host ? "Utilisez cette adresse depuis un réseau extérieur, après vérification de son ouverture."
      : "L’accès public n’est pas encore configuré. Utilisez le réseau local du propriétaire du pool.";
    $("connection-availability").lastElementChild.textContent = currentlyReady()
      ? "Le suivi indique un pool disponible. Vérifiez les shares acceptées après connexion."
      : "Disponibilité non confirmée en direct. Gardez votre pool actuel pendant la préparation.";
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
      $("sample-count").textContent = "Données non disponibles";
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
    const formatTime = (value) => new Intl.DateTimeFormat("fr-FR", { hour: "2-digit", minute: "2-digit", timeZone: "Europe/Paris" }).format(new Date(value));
    for (const [x, anchor, time] of [[48, "start", points[0].time], [630, "end", points.at(-1).time]]) {
      const text = svgElement("text", { x, y: 165, "text-anchor": anchor, class: "chart-label" });
      text.textContent = formatTime(time);
      svg.appendChild(text);
    }
    $("sample-count").textContent = points.length + " mesures · heure de Paris";
  }
  function renderStatus() {
    const data = statusData;
    if (!data) return;
    const manual = data.source !== "live";
    const node = data.node || {};
    const pool = data.pool || {};
    const progress = finite(node.verificationProgress) ? Math.min(1, node.verificationProgress) * 100 : null;
    statusStale = !manual && (!Number.isFinite(Date.parse(data.updatedAt)) || Date.now() - Date.parse(data.updatedAt) > 180000 || Date.parse(data.updatedAt) > Date.now() + 60000);
    const unreliable = statusFailed || statusStale;
    if (unreliable) {
      badge("pool-badge", "Suivi indisponible", "neutral");
      $("data-notice").textContent = "Le suivi est indisponible ou trop ancien. Les dernières données conservées ne confirment pas la disponibilité du pool.";
    } else if (manual) {
      badge("pool-badge", "Relevé manuel", "amber");
      $("data-notice").textContent = "Dernier relevé manuel : " + dateLabel(data.updatedAt) + ". Ces informations ne sont pas actualisées en direct ; les mesures inconnues restent vides.";
    } else {
      badge("pool-badge", currentlyReady() ? "Pool disponible" : "Pool en préparation", currentlyReady() ? "green" : "amber");
      $("data-notice").textContent = "Suivi connecté · relevé du " + new Intl.DateTimeFormat("fr-FR", { dateStyle: "medium", timeStyle: "short", timeZone: "Europe/Paris" }).format(new Date(data.updatedAt)) + " (heure de Paris).";
    }
    $("metric-hashrate").textContent = hashrate(pool.hashrateHps);
    $("metric-workers").textContent = finite(pool.workers) ? number.format(pool.workers) : "—";
    $("metric-best").textContent = finite(pool.bestShare) ? compact.format(pool.bestShare) : "—";
    $("metric-blocks").textContent = finite(pool.blocksFound) ? number.format(pool.blocksFound) : "—";
    $("node-blocks").textContent = finite(node.blocks) ? number.format(node.blocks) : "—";
    $("node-headers").textContent = finite(node.headers) ? number.format(node.headers) : "—";
    $("sync-value").replaceChildren(document.createTextNode(progress === null ? "—" : new Intl.NumberFormat("fr-FR", { minimumFractionDigits: 2, maximumFractionDigits: 2 }).format(progress)));
    const percent = document.createElement("span"); percent.textContent = " %"; $("sync-value").appendChild(percent);
    $("sync-fill").style.width = (progress || 0) + "%";
    $("sync-progress").setAttribute("aria-valuenow", String(progress || 0));
    const nodeSynced = node.initialBlockDownload === false;
    badge("node-badge", unreliable ? "Dernier état connu" : nodeSynced ? (manual ? "Synchronisé · relevé" : "Synchronisé") : "Synchronisation initiale", unreliable ? "neutral" : nodeSynced ? "green" : "amber");
    $("node-note").textContent = unreliable ? "État actuel non confirmé. Vérifiez le serveur avant de connecter un mineur." : manual ? "État du dernier relevé. Le serveur prévoit un démarrage du pool après synchronisation." : nodeSynced ? "Vérification initiale terminée. La disponibilité Stratum dépend aussi de CKPool." : "Le pool attend la fin de la synchronisation complète du nœud.";
    $("activity-state").textContent = unreliable ? "Dernières données conservées" : finite(pool.hashrateHps) ? (manual ? "Dernière mesure manuelle" : "Mesures reçues") : "Aucune mesure reçue";
    $("faq-availability").textContent = unreliable ? "Le suivi est actuellement indisponible ou trop ancien. Vérifiez le serveur avant de connecter un mineur." : manual ? "Le dernier relevé manuel indique un pool en préparation. Ce site ne confirme pas sa disponibilité actuelle tant que le suivi en direct n’est pas connecté." : currentlyReady() ? "Le suivi indique un nœud synchronisé et un pool disponible. Après connexion, vérifiez la réception de travail et les shares acceptées dans votre mineur." : "Le suivi indique un pool en préparation. Gardez votre mineur sur son pool actuel tant que le serveur n’est pas prêt.";
    renderChart(data.history);
    renderEndpoint();
  }
  async function loadStatus(manualClick = false) {
    if (loading) return;
    loading = true;
    $("refresh-status").disabled = true;
    try {
      const url = new URL(config.statusUrl || "./data/status.json", location.href);
      if (url.origin !== location.origin && url.protocol !== "https:") throw new Error("HTTPS required");
      const response = await fetch(url, { cache: "no-store", credentials: "omit", signal: AbortSignal.timeout(10000) });
      if (!response.ok) throw new Error("Status unavailable");
      const data = await response.json();
      if (data.schemaVersion !== 1 || !data.node || !data.pool || !["manual", "live"].includes(data.source)) throw new Error("Invalid status schema");
      statusData = data;
      statusFailed = false;
      latestPoll = Date.now();
      renderStatus();
      if (manualClick) toast(data.source === "manual" ? "Relevé manuel chargé. Le suivi en direct n’est pas encore connecté." : statusStale ? "Relevé trop ancien : disponibilité non confirmée." : "Suivi actualisé.");
    } catch {
      statusFailed = true;
      if (statusData) renderStatus();
      else {
        badge("pool-badge", "Suivi indisponible", "neutral");
        badge("node-badge", "État inconnu", "neutral");
        $("data-notice").textContent = "Le relevé n’a pas pu être chargé. La disponibilité actuelle du pool n’est pas confirmée.";
        $("node-note").textContent = "Aucune information de suivi disponible.";
        renderEndpoint();
      }
      if (manualClick) toast("Impossible de charger le suivi. Réessayez plus tard.");
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
    if (!host) error = "L’adresse de connexion n’est pas encore configurée.";
    else if (!addressFormat.test(address)) error = "Vérifiez le format de votre adresse Bitcoin BTC : bc1…, 1… ou 3….";
    else if (worker && !/^[a-zA-Z0-9_-]{1,32}$/.test(worker)) { error = "Nom du mineur : 32 caractères maximum, lettres, chiffres, tiret ou underscore."; focus = "worker-name"; }
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
    copy("Serveur : " + settings.host + "\nPort : " + settings.port + "\nUtilisateur : " + settings.username + "\nMot de passe : x\nURL Stratum : stratum+tcp://" + settings.host + ":" + settings.port);
  });
  document.querySelector(".menu-toggle").addEventListener("click", (event) => {
    const button = event.currentTarget;
    const open = button.getAttribute("aria-expanded") !== "true";
    button.setAttribute("aria-expanded", String(open));
    button.setAttribute("aria-label", open ? "Fermer le menu" : "Ouvrir le menu");
    $("main-nav").classList.toggle("open", open);
  });
  document.querySelectorAll(".main-nav a").forEach((link) => link.addEventListener("click", () => {
    $("main-nav").classList.remove("open");
    document.querySelector(".menu-toggle").setAttribute("aria-expanded", "false");
    document.querySelector(".menu-toggle").setAttribute("aria-label", "Ouvrir le menu");
  }));
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      $("main-nav").classList.remove("open");
      document.querySelector(".menu-toggle").setAttribute("aria-expanded", "false");
      document.querySelector(".menu-toggle").setAttribute("aria-label", "Ouvrir le menu");
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
    if (statusData?.source === "live") loadStatus();
    if (statusData) renderStatus();
  }, Math.max(15000, Number(config.pollIntervalMs) || 60000));
  renderEndpoint();
  loadStatus();
})();
