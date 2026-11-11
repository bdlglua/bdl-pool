use crate::{address, chart, dom::*};
use regex::Regex;
use serde_json::{json, Value};
use std::cell::RefCell;
use wasm_bindgen::{JsCast, JsValue};

thread_local! {static APP:RefCell<Option<App>>=const{RefCell::new(None)};}
pub fn bch() -> bool {
    current_url().search_params().get("coin").as_deref() == Some("bch")
}
pub fn config() -> Value {
    APP.with(|a| {
        a.borrow()
            .as_ref()
            .map(|a| a.config.clone())
            .unwrap_or_else(|| serde_json::from_str(include_str!("../../../config.json")).unwrap())
    })
}
fn with(f: impl FnOnce(&mut App)) {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            f(app);
        }
    });
}
struct App {
    config: Value,
    translations: Value,
    protocol: String,
    settings: Option<Value>,
    status: Option<Value>,
    last_node: Option<Value>,
    storage_key: String,
    failed: bool,
    stale: bool,
    node_current: bool,
    loading: bool,
    latest: f64,
    toast_timer: i32,
}
impl App {
    fn t(&self, value: &str) -> String {
        let text = if lang() {
            self.translations[value].as_str().unwrap_or(value)
        } else {
            value
        };
        if bch() {
            Regex::new(r"Bitcoin( Cash)?")
                .unwrap()
                .replace_all(&text.replace("Bitcoin Core", "BCHN"), "Bitcoin Cash")
                .to_string()
                .replace("BTC", "BCH")
        } else {
            text.into()
        }
    }
    fn set(&self, id: &str, key: &str) {
        text(id, &self.t(key));
    }
    fn toast(&mut self, key: &str) {
        window().clear_timeout_with_handle(self.toast_timer);
        text("toast", &self.t(key));
        hidden("toast", false);
        self.toast_timer = later(4200, || hidden("toast", true));
    }
    fn ready(&self) -> bool {
        self.node_current
            && !self.failed
            && !self.stale
            && self
                .status
                .as_ref()
                .map(|v| {
                    matches!(v["source"].as_str(), Some("live" | "snapshot"))
                        && v["pool"]["ready"] == true
                        && v["node"]["initialBlockDownload"] == false
                })
                .unwrap_or(false)
    }
    fn host(&self) -> String {
        string(&self.config["publicHost"]).trim().into()
    }
    fn v2(&self) -> bool {
        self.protocol == "v2"
    }
    fn v2_configured(&self) -> bool {
        self.config["sv2"]["enabled"] == true
            && Regex::new(r"^[1-9A-HJ-NP-Za-km-z]{40,70}$")
                .unwrap()
                .is_match(&string(&self.config["sv2"]["authorityPublicKey"]))
    }
    fn port(&self) -> Value {
        let value = if self.v2() {
            &self.config["sv2"]["publicPort"]
        } else {
            &self.config["publicPort"]
        };
        if finite(value).filter(|v| *v > 0.0).is_some() {
            value.clone()
        } else if bch() {
            Value::Null
        } else {
            json!(if self.v2() { 3336 } else { 3333 })
        }
    }
    fn url(&self) -> String {
        let host = self.host();
        if host.is_empty() || self.port().is_null() || (self.v2() && !self.v2_configured()) {
            return String::new();
        }
        if self.v2() {
            format!(
                "stratum2+tcp://{host}:{}/{}",
                self.port(),
                string(&self.config["sv2"]["authorityPublicKey"])
            )
        } else {
            format!("stratum+tcp://{host}:{}", self.port())
        }
    }
    fn endpoint(&self) {
        if get("endpoint-host").is_none() {
            return;
        }
        let inactive = self.v2() && !self.v2_configured();
        let host = self.host();
        text(
            "endpoint-host",
            &if inactive {
                self.t("Non activé")
            } else if host.is_empty() {
                self.t("Non configuré")
            } else {
                host.clone()
            },
        );
        text(
            "endpoint-port",
            &if inactive || self.port().is_null() {
                "—".into()
            } else {
                string(&self.port())
            },
        );
        hidden("endpoint-key-row", !self.v2());
        text(
            "endpoint-key",
            &if self.v2() && !inactive {
                string(&self.config["sv2"]["authorityPublicKey"])
            } else {
                "—".into()
            },
        );
        for id in ["copy-host", "copy-port"] {
            disabled(id, host.is_empty() || inactive);
        }
        disabled("copy-key", !self.v2() || inactive);
        disabled("copy-url", self.url().is_empty());
        if let Some(button) = query("#miner-form button[type=submit]") {
            let _ = button.toggle_attribute_with_force("disabled", host.is_empty() || inactive);
        }
        hidden("protocol-note", !inactive);
        text(
            "protocol-note",
            &if inactive {
                self.t("Stratum V2 non activé.")
            } else {
                String::new()
            },
        );
        let mut availability = self.t("État du pool non confirmé.");
        if bch() && host.is_empty() {
            availability = tr(
                "Serveur BCH pas encore configuré.",
                "BCH endpoint not configured yet.",
            )
            .into();
        }
        if self.node_current && !self.failed && !self.stale {
            availability = self.t(
                if self
                    .status
                    .as_ref()
                    .map(|v| v["node"]["initialBlockDownload"] == true)
                    .unwrap_or(false)
                {
                    "Bitcoin en synchronisation."
                } else if self.v2() {
                    "Connexion Stratum V2 à vérifier sur le mineur."
                } else if self.ready() {
                    "Travail Stratum V1 reçu au dernier contrôle."
                } else {
                    "Connexion Stratum V1 à vérifier."
                },
            );
        }
        hidden("connection-availability", inactive);
        if let Some(el) = get("connection-availability").and_then(|e| e.last_element_child()) {
            el.set_text_content(Some(&availability));
        }
        if let Some(link) = query(".language-switch") {
            if let Ok(url) = web_sys::Url::new_with_base(
                &link.get_attribute("href").unwrap_or_default(),
                &current_url().href(),
            ) {
                url.search_params().set("protocol", &self.protocol);
                let _ = link.set_attribute("href", &url.href());
            }
        }
        for button in all("[data-protocol]") {
            let selected = button.get_attribute("data-protocol").as_deref() == Some(&self.protocol);
            class(&button, "selected", selected);
            let _ = button.set_attribute("aria-pressed", if selected { "true" } else { "false" });
        }
    }
    fn select_node(&mut self, data: &Value) -> (Value, Value, bool) {
        let incoming = &data["node"];
        let checks = &data["checks"]["bitcoin"];
        let stamp = if incoming["updatedAt"].is_string() {
            incoming["updatedAt"].clone()
        } else if checks == "stale" {
            Value::Null
        } else {
            data["updatedAt"].clone()
        };
        if data["source"] == "manual" {
            return (incoming.clone(), stamp, false);
        }
        let valid = known_node(incoming) && valid_date(&stamp);
        if valid
            && checks != "unavailable"
            && self
                .last_node
                .as_ref()
                .map(|n| crate::dom::stamp(&stamp) >= crate::dom::stamp(&n["updatedAt"]))
                .unwrap_or(true)
        {
            let mut node = incoming.clone();
            node["updatedAt"] = stamp.clone();
            store(&self.storage_key, &node.to_string());
            self.last_node = Some(node);
        }
        if let Some(last) = &self.last_node {
            if !valid || crate::dom::stamp(&last["updatedAt"]) > crate::dom::stamp(&stamp) {
                return (last.clone(), last["updatedAt"].clone(), true);
            }
        }
        (
            incoming.clone(),
            stamp,
            !valid || matches!(checks.as_str(), Some("stale" | "unavailable")),
        )
    }
    fn status(&mut self) {
        let Some(data) = self.status.clone() else {
            return;
        };
        let manual = data["source"] == "manual";
        let snapshot = data["source"] == "snapshot";
        let (node, updated, was_retained) = self.select_node(&data);
        let pool = &data["pool"];
        let max = if snapshot { 900000.0 } else { 180000.0 };
        self.stale = !manual && !fresh(&data["updatedAt"], max);
        let unreliable = self.failed || self.stale;
        let unknown = !known_node(&node);
        let old = !manual && (!valid_date(&updated) || now() - stamp(&updated) > max);
        let retained = was_retained || unreliable || old;
        self.node_current = !manual && !unknown && !retained;
        if unreliable {
            self.badge("pool-badge", "Suivi indisponible", "neutral");
            text(
                "data-notice",
                &(self.t("Suivi indisponible.")
                    + &self.t(" · dernier relevé : ")
                    + &self.date(&data["updatedAt"])),
            );
        } else if manual {
            self.badge("pool-badge", "Relevé manuel", "amber");
            text(
                "data-notice",
                &(self.t("Dernier relevé manuel : ")
                    + &date(&data["updatedAt"], json!({"dateStyle":"medium"}))),
            );
        } else if retained || unknown {
            self.badge("pool-badge", "Actualisation en attente", "amber");
            text(
                "data-notice",
                &if unknown {
                    self.t("Aucun relevé du nœud n’est encore disponible.")
                } else {
                    self.t("Dernière lecture réussie : ")
                        + &self.date(&updated)
                        + &self.t(" · actualisation en attente")
                },
            );
        } else {
            let ready = self.ready();
            let label = if ready {
                "Pool disponible"
            } else if node["initialBlockDownload"] == true {
                "Synchronisation initiale"
            } else {
                "Disponibilité à vérifier"
            };
            self.badge("pool-badge", label, if ready { "green" } else { "amber" });
            text(
                "data-notice",
                &if snapshot {
                    self.t("Dernière mise à jour : ")
                        + &self.date(&data["updatedAt"])
                        + &self.t(" · toutes les 5 min")
                } else {
                    self.t("Suivi connecté · relevé du ") + &self.date(&data["updatedAt"])
                },
            );
        }
        text("metric-hashrate", &hash(&pool["hashrateHps"]));
        text("metric-workers", &number(&pool["workers"]));
        text(
            "metric-best",
            &finite(&pool["bestShare"])
                .map(|v| format(v, json!({"notation":"compact","maximumFractionDigits":2})))
                .unwrap_or_else(|| "—".into()),
        );
        text("metric-blocks", &number(&pool["blocksFound"]));
        for (key, id) in [("blocks", "node-blocks"), ("headers", "node-headers")] {
            text(id, &number(&node[key]));
        }
        let progress = finite(&node["verificationProgress"]).map(|p| p.min(1.0) * 100.0);
        let label = progress
            .map(|p| {
                format(
                    p,
                    json!({"minimumFractionDigits":2,"maximumFractionDigits":2}),
                )
            })
            .unwrap_or_else(|| "—".into());
        html("sync-value", &format!("{}<span> %</span>", escape(&label)));
        if let Some(el) = get("sync-fill").and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()) {
            let _ = el
                .style()
                .set_property("width", &format!("{}%", progress.unwrap_or(0.0)));
        }
        attr(
            "sync-progress",
            "aria-valuenow",
            &progress.unwrap_or(0.0).to_string(),
        );
        let synced = node["initialBlockDownload"] == false;
        self.badge(
            "node-badge",
            if unknown {
                "En attente d’un relevé"
            } else if retained {
                "Dernier état connu"
            } else if synced {
                if manual {
                    "Synchronisé · relevé"
                } else {
                    "Synchronisé"
                }
            } else {
                "Synchronisation initiale"
            },
            if retained || unknown {
                "neutral"
            } else if synced {
                "green"
            } else {
                "amber"
            },
        );
        if unknown {
            self.set("node-note", "Relevé du nœud indisponible.");
        } else if retained {
            text(
                "node-note",
                &(if old {
                    self.t("Relevé ancien. ")
                } else {
                    String::new()
                } + &self.t("Dernière lecture réussie : ")
                    + &self.date(&updated)
                    + "."),
            );
        } else {
            self.set(
                "node-note",
                if manual {
                    "Relevé manuel. État actuel non confirmé."
                } else if synced {
                    "Synchronisation terminée."
                } else {
                    "BDLPool démarrera après la synchronisation."
                },
            );
        }
        self.set(
            "activity-state",
            if unreliable {
                "Dernières données conservées"
            } else if finite(&pool["hashrateHps"]).is_some() {
                if manual {
                    "Dernière mesure manuelle"
                } else {
                    "Mesures reçues"
                }
            } else {
                "Aucune mesure reçue"
            },
        );
        self.set(
            "faq-availability",
            if unreliable || manual || retained || unknown {
                "État actuel non confirmé par le suivi."
            } else if self.ready() {
                "Du travail Stratum V1 a été reçu au dernier contrôle."
            } else if synced {
                "Le nœud est synchronisé. La connexion Stratum reste à vérifier."
            } else {
                "Bitcoin est en synchronisation. BDLPool démarrera ensuite."
            },
        );
        if self.node_current && data["checks"]["stratum"] == "subscribed_no_job" {
            self.set(
                "faq-availability",
                "Stratum V1 répond. Aucun travail reçu au dernier contrôle.",
            );
        }
        self.chart(&data["history"]);
        self.endpoint();
    }
    fn badge(&self, id: &str, key: &str, style: &str) {
        self.set(id, key);
        attr(id, "class", &format!("badge {style}"));
    }
    fn date(&self, value: &Value) -> String {
        if !stamp(value).is_finite() {
            self.t("date non renseignée")
        } else {
            date(value, json!({"dateStyle":"medium","timeStyle":"short"}))
                + &self.t(" (heure de Paris)")
        }
    }
    fn chart(&self, history: &Value) {
        if get("hash-chart").is_none() {
            return;
        }
        let mut points = history
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| Some((stamp(&v["time"]), Some(finite(&v["hashrateHps"])?))))
            .filter(|(t, _)| t.is_finite())
            .collect::<Vec<_>>();
        points.sort_by(|a, b| a.0.total_cmp(&b.0));
        let points = &points[points.len().saturating_sub(120)..];
        hidden("hash-chart", true);
        hidden("pool-chart-frame", points.len() < 2);
        hidden("chart-empty", points.len() >= 2);
        if points.len() < 2 {
            hidden("cryptoChart", true);
            self.set("sample-count", "Données non disponibles");
            return;
        }
        text(
            "sample-count",
            &(points.len().to_string() + &self.t(" mesures · heure de Paris")),
        );
        chart::render("cryptoChart", points, 750000.0);
    }
    fn clear(&mut self) {
        self.settings = None;
        hidden("miner-result", true);
        hidden("form-error", true);
        remove_attr("btc-address", "aria-invalid");
        remove_attr("worker-name", "aria-invalid");
    }
    fn submit(&mut self) {
        self.clear();
        let entered = value("btc-address").trim().to_string();
        let address = address::canonical(&entered, bch());
        let worker = value("worker-name").trim().to_string();
        let mut target = "btc-address";
        let error = if self.v2() && !self.v2_configured() {
            self.t("Stratum V2 n’est pas encore activé. Utilisez Stratum V1 pour le moment.")
        } else if self.host().is_empty() {
            self.t("L’adresse de connexion n’est pas encore configurée.")
        } else if address.is_none() {
            if bch() {
                tr(
                    "Vérifiez votre adresse CashAddr Bitcoin Cash et sa somme de contrôle.",
                    "Check your Bitcoin Cash mainnet CashAddr and its checksum.",
                )
                .into()
            } else {
                self.t("Vérifiez le format de votre adresse Bitcoin BTC : bc1…, 1… ou 3….")
            }
        } else if !worker.is_empty()
            && !Regex::new(r"^[a-zA-Z0-9_-]{1,32}$")
                .unwrap()
                .is_match(&worker)
        {
            target = "worker-name";
            self.t("Nom du mineur : 32 caractères maximum, lettres, chiffres, tiret ou underscore.")
        } else {
            String::new()
        };
        if !error.is_empty() {
            text("form-error", &error);
            hidden("form-error", false);
            attr(target, "aria-invalid", "true");
            focus(target);
            return;
        }
        let username = address.unwrap_or_default()
            + &if worker.is_empty() {
                String::new()
            } else {
                format!(".{worker}")
            };
        let key = if self.v2() {
            self.config["sv2"]["authorityPublicKey"].clone()
        } else {
            Value::Null
        };
        let password = if self.v2() { Value::Null } else { json!("x") };
        text("result-url", &self.url());
        text(
            "result-password",
            &if password.is_null() {
                self.t("Non utilisé")
            } else {
                "x".into()
            },
        );
        text("result-host", &self.host());
        text("result-port", &string(&self.port()));
        hidden("result-key-row", key.is_null());
        text("result-key", &string(&key));
        text("result-user", &username);
        hidden("miner-result", false);
        self.settings = Some(
            json!({"host":self.host(),"port":self.port(),"url":self.url(),"protocol":self.protocol,"authorityPublicKey":key,"username":username,"password":password}),
        );
    }
}
fn known_node(node: &Value) -> bool {
    matches!(
        node["chain"].as_str(),
        Some("main" | "test" | "testnet4" | "signet" | "regtest")
    ) && ["blocks", "headers"]
        .iter()
        .all(|k| finite(&node[k]).filter(|v| v.fract() == 0.0).is_some())
        && finite(&node["verificationProgress"])
            .filter(|v| *v <= 1.0)
            .is_some()
        && node["initialBlockDownload"].is_boolean()
}
fn valid_date(value: &Value) -> bool {
    value.is_string() && stamp(value).is_finite() && stamp(value) <= now() + 60000.0
}
fn load(manual: bool) {
    let mut url = None;
    with(|app| {
        if app.loading {
            return;
        }
        app.loading = true;
        disabled("refresh-status", true);
        attr("refresh-status", "aria-busy", "true");
        let raw = app.config["statusUrl"]
            .as_str()
            .unwrap_or("/data/status.json");
        if let Ok(dest) = web_sys::Url::new_with_base(raw, &current_url().href()) {
            if dest.hostname() == "raw.githubusercontent.com" {
                dest.search_params().set(
                    "v",
                    &if manual {
                        now().to_string()
                    } else {
                        (now() / 60000.0).floor().to_string()
                    },
                );
            }
            url = Some(dest.href());
        }
    });
    let Some(url) = url else {
        return;
    };
    run(async move {
        let result = fetch(&url, 10000).await;
        with(|app| {
            let valid = result.ok().filter(|d| {
                d["schemaVersion"] == 1
                    && d["node"].is_object()
                    && d["pool"].is_object()
                    && matches!(d["source"].as_str(), Some("manual" | "live" | "snapshot"))
                    && d["coin"].as_str().unwrap_or("btc") == if bch() { "bch" } else { "btc" }
            });
            if let Some(data) = valid {
                app.status = Some(data);
                app.failed = false;
                app.latest = now();
                app.status();
                if manual {
                    app.toast(
                        if app
                            .status
                            .as_ref()
                            .map(|d| d["source"] == "manual")
                            .unwrap_or(false)
                        {
                            "Relevé manuel chargé. Le suivi en direct n’est pas encore connecté."
                        } else if app.stale {
                            "Relevé trop ancien : disponibilité non confirmée."
                        } else {
                            "Suivi actualisé."
                        },
                    );
                }
            } else {
                app.failed = true;
                if app.status.is_none() {
                    if let Some(node) = &app.last_node {
                        app.status = Some(
                            json!({"schemaVersion":1,"source":"snapshot","updatedAt":node["updatedAt"],"node":node,"pool":{"ready":false},"history":[],"checks":{"bitcoin":"stale"}}),
                        );
                    }
                }
                if app.status.is_some() {
                    app.status();
                } else {
                    app.badge("pool-badge", "Suivi indisponible", "neutral");
                    app.badge("node-badge", "En attente d’un relevé", "neutral");
                    app.set("data-notice","Le relevé n’a pas pu être chargé. La disponibilité actuelle du pool n’est pas confirmée.");
                    app.set("node-note", "Aucune information de suivi disponible.");
                    app.endpoint();
                }
                if manual {
                    app.toast("Impossible de charger le suivi. Réessayez plus tard.");
                }
            }
            app.loading = false;
            disabled("refresh-status", false);
            attr("refresh-status", "aria-busy", "false");
        });
    });
}
pub fn init() {
    let root: Value =
        serde_json::from_str(include_str!("../../../config.json")).expect("public configuration");
    let mut config = if bch() {
        root["coins"]["bch"].clone()
    } else {
        root.clone()
    };
    config["pollIntervalMs"] = root["pollIntervalMs"].clone();
    if let Some(body) = document().body() {
        let _ = body.set_attribute("data-coin", if bch() { "bch" } else { "btc" });
    }
    set_value("pool-coin", if bch() { "bch" } else { "btc" });
    on_id("pool-coin", "change", |_| {
        let url = current_url();
        url.search_params().set("coin", &value("pool-coin"));
        url.search_params().delete("protocol");
        let _ = window().location().assign(&url.href());
    });
    if bch() {
        for link in all("a[href]") {
            if let Ok(url) = web_sys::Url::new_with_base(
                &link.get_attribute("href").unwrap_or_default(),
                &current_url().href(),
            ) {
                if url.origin() == current_url().origin() && url.pathname().ends_with('/') {
                    url.search_params().set("coin", "bch");
                    let _ = link.set_attribute("href", &url.href());
                }
            }
        }
        if let Some(main) = get("main") {
            translate_bch(main.as_ref(), &root, &config);
        }
        for id in ["btc-address", "miner-lookup-address"] {
            attr(id, "placeholder", "bitcoincash:q…");
        }
        if let Some(el) = query(".hero-description") {
            el.set_text_content(Some(tr(
                "Minage Bitcoin Cash en solo · Stratum V1",
                "Bitcoin Cash solo mining · Stratum V1",
            )));
        }
        for el in all("a[href*=\"protocol=v2\"], [data-protocol=v2]") {
            let _ = el.set_attribute("hidden", "");
            let _ = el.set_attribute("disabled", "");
        }
        text("faq-connection",tr("Choisissez Bitcoin Cash sur la page Connexion pour voir son serveur Stratum V1 dédié. BCH utilise son propre réseau et une adresse de réception distincte.","Choose Bitcoin Cash on the Connection page to see its dedicated Stratum V1 endpoint. BCH uses a separate receiving address and network."));
    }
    let storage_key = format!(
        "bdl-pool:last-good-node:v1:{}",
        config["statusUrl"].as_str().unwrap_or("/data/status.json")
    );
    let last_node = read_storage(&storage_key)
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .filter(|v| known_node(v) && valid_date(&v["updatedAt"]));
    let protocol =
        if !bch() && current_url().search_params().get("protocol").as_deref() == Some("v2") {
            "v2"
        } else {
            "v1"
        }
        .into();
    APP.with(|cell| {
        *cell.borrow_mut() = Some(App {
            config,
            translations: serde_json::from_str(include_str!("translations.json")).unwrap(),
            protocol,
            settings: None,
            status: None,
            last_node,
            storage_key,
            failed: false,
            stale: false,
            node_current: false,
            loading: false,
            latest: 0.0,
            toast_timer: 0,
        })
    });
    click("refresh-status", || load(true));
    for (id, key) in [
        ("copy-host", "host"),
        ("copy-port", "port"),
        ("copy-key", "key"),
        ("copy-url", "url"),
    ] {
        click(id, move || {
            let mut value = String::new();
            with(|a| {
                value = match key {
                    "host" => a.host(),
                    "port" => string(&a.port()),
                    "key" => string(&a.config["sv2"]["authorityPublicKey"]),
                    _ => a.url(),
                }
            });
            copy_toast(value);
        });
    }
    for button in all("[data-protocol]") {
        let protocol = button.get_attribute("data-protocol").unwrap_or_default();
        on(button.as_ref(), "click", move |_| {
            with(|a| {
                a.protocol = protocol.clone();
                let url = current_url();
                url.search_params().set("protocol", &protocol);
                let _ = window()
                    .history()
                    .and_then(|h| h.replace_state_with_url(&JsValue::NULL, "", Some(&url.href())));
                a.clear();
                a.endpoint();
            });
        });
    }
    for id in ["btc-address", "worker-name"] {
        on_id(id, "input", |_| with(|a| a.clear()));
    }
    on_id("miner-form", "submit", |event| {
        event.prevent_default();
        with(|a| a.submit());
    });
    click("copy-settings", || {
        let mut output = None;
        with(|a| {
            if let Some(s) = &a.settings {
                output = Some(
                    a.t("Protocole : ")
                        + if s["protocol"] == "v2" {
                            "Stratum V2"
                        } else {
                            "Stratum V1"
                        }
                        + "\n"
                        + &a.t("URL Stratum : ")
                        + &string(&s["url"])
                        + "\n"
                        + &a.t("Adresse : ")
                        + &string(&s["host"])
                        + "\n"
                        + &a.t("Port : ")
                        + &string(&s["port"])
                        + &if s["authorityPublicKey"].is_null() {
                            String::new()
                        } else {
                            "\n".to_string()
                                + &a.t("Clé publique de chiffrement : ")
                                + &string(&s["authorityPublicKey"])
                        }
                        + "\n"
                        + tr("Utilisateur : ", "Username: ")
                        + &string(&s["username"])
                        + "\n"
                        + &a.t("Mot de passe : ")
                        + &if s["password"].is_null() {
                            a.t("Non utilisé")
                        } else {
                            string(&s["password"])
                        },
                );
            }
        });
        if let Some(value) = output {
            copy_toast(value);
        }
    });
    on(document().as_ref(), "visibilitychange", |_| {
        let mut refresh = false;
        with(|a| refresh = !document().hidden() && now() - a.latest > 60000.0);
        if refresh {
            load(false);
        }
    });
    let mut poll = 60000;
    with(|a| {
        poll = a.config["pollIntervalMs"]
            .as_i64()
            .unwrap_or(60000)
            .max(15000) as i32
    });
    interval(poll, || {
        if !document().hidden() {
            load(false);
            with(|a| a.status());
        }
    });
    on(document().as_ref(), "bdl-theme-change", |_| {
        with(|a| {
            if let Some(data) = &a.status {
                a.chart(&data["history"]);
            }
        })
    });
    with(|a| {
        if let Some(node) = &a.last_node {
            a.status = Some(
                json!({"schemaVersion":1,"source":"snapshot","updatedAt":node["updatedAt"],"node":node,"pool":{"ready":false},"history":[],"checks":{"bitcoin":"stale"}}),
            );
            a.status();
        }
        a.endpoint();
    });
    if bch() && get("endpoint-host").is_some() {
        let base = crate::pool::config()["apiBaseUrl"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        run(async move {
            if let Ok(data) = fetch(&format!("{base}/stratum"), 8000).await {
                if data["apiVersion"] == "1"
                    && data["coin"] == "bch"
                    && Regex::new(r"^[a-z0-9][a-z0-9.-]*$")
                        .unwrap()
                        .is_match(&string(&data["v1"]["host"]))
                    && data["v1"]["port"]
                        .as_u64()
                        .map(|p| (1..=65535).contains(&p))
                        .unwrap_or(false)
                {
                    with(|a| {
                        a.config["publicHost"] = data["v1"]["host"].clone();
                        a.config["publicPort"] = data["v1"]["port"].clone();
                        a.endpoint();
                    });
                }
            }
        });
    }
    if ["home", "statistics", "connect", "faq"].contains(
        &document()
            .body()
            .and_then(|b| b.get_attribute("data-page"))
            .unwrap_or_default()
            .as_str(),
    ) {
        load(false);
    }
}
fn copy_toast(value: String) {
    run(async move {
        let ok = copy(&value).await;
        with(|a| {
            a.toast(if ok {
                "Copié dans le presse-papiers."
            } else {
                "Copie indisponible : sélectionnez le texte affiché pour le copier."
            })
        });
    });
}
fn translate_bch(node: &web_sys::Node, root: &Value, config: &Value) {
    if node.node_type() == web_sys::Node::TEXT_NODE {
        let text = node.text_content().unwrap_or_default();
        let translated = Regex::new(r"Bitcoin( Cash)?")
            .unwrap()
            .replace_all(&text, "Bitcoin Cash")
            .to_string();
        let translated = Regex::new(r"\bBTC\b")
            .unwrap()
            .replace_all(&translated, "BCH")
            .replace("VOTRE_ADRESSE_BTC", "VOTRE_ADRESSE_BCH")
            .replace("YOUR_BTC_ADDRESS", "YOUR_BCH_ADDRESS")
            .replace(
                root["apiBaseUrl"].as_str().unwrap_or("\0"),
                config["apiBaseUrl"].as_str().unwrap_or_default(),
            );
        node.set_text_content(Some(&translated));
    } else {
        let children = node.child_nodes();
        for i in 0..children.length() {
            if let Some(child) = children.item(i) {
                translate_bch(&child, root, config);
            }
        }
    }
}
