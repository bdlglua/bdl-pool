use crate::{address, dom::*, pool};
use regex::Regex;
use serde_json::{json, Value};
use std::cell::RefCell;

thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
#[derive(Default)]
struct State {
    online: Option<Value>,
    online_offset: usize,
    online_busy: bool,
    selected: Option<String>,
    selected_offset: usize,
    data: Option<Value>,
    failed: bool,
    busy: bool,
    generation: u64,
    controller: Option<web_sys::AbortController>,
}
fn with(f: impl FnOnce(&mut State)) {
    STATE.with(|s| f(&mut s.borrow_mut()));
}
fn msg(id: &str, value: &str, error: bool) {
    text(id, value);
    hidden(id, value.is_empty());
    if let Some(el) = get(id) {
        class(&el, "error", error);
    }
}
fn masked(value: &str) -> String {
    if value.is_empty() {
        "—".into()
    } else if value.contains('…') {
        value.chars().take(40).collect()
    } else {
        format!(
            "{}…{}",
            value.chars().take(6).collect::<String>(),
            value
                .chars()
                .rev()
                .take(4)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>()
        )
    }
}
fn compact(value: &Value) -> String {
    finite(value)
        .map(|v| {
            format(
                v,
                json!({"notation":"compact","maximumSignificantDigits":4}),
            )
        })
        .unwrap_or_else(|| "—".into())
}
fn best(data: &Value) -> Value {
    [finite(&data["bestEver"]), finite(&data["bestShare"])]
        .into_iter()
        .flatten()
        .max_by(f64::total_cmp)
        .map(|v| json!(v))
        .unwrap_or(Value::Null)
}
fn cell(value: &str, class: &str) -> String {
    format!(
        "<td{}>{}</td>",
        if class.is_empty() {
            String::new()
        } else {
            format!(" class=\"{}\"", escape(class))
        },
        escape(value)
    )
}
fn device(devices: &Value, known: bool) -> String {
    let Some(items) = devices.as_array().filter(|d| !d.is_empty() && known) else {
        return cell("—", "device-name");
    };
    let mut labels = Vec::new();
    for item in items.iter().take(20) {
        let label = item["model"]
            .as_str()
            .or_else(|| item["software"].as_str())
            .unwrap_or(tr("Non identifié", "Unidentified"));
        if !labels.contains(&label) {
            labels.push(label);
        }
    }
    let label = labels
        .iter()
        .take(3)
        .copied()
        .collect::<Vec<_>>()
        .join(", ")
        + &if labels.len() > 3 {
            format!(" +{}", labels.len() - 3)
        } else {
            String::new()
        };
    let mut detail = Vec::new();
    let first = &items[0];
    if first["model"].is_null() && first["software"].is_string() {
        detail.push(tr("Modèle non fourni", "Model not provided").to_string());
    }
    if let Some(v) = first["version"].as_str() {
        detail.push(format!("v{v}"));
    }
    format!(
        "<td class=\"device-name\">{}{}</td>",
        escape(&label),
        if detail.is_empty() {
            String::new()
        } else {
            format!("<small>{}</small>", escape(&detail.join(" · ")))
        }
    )
}
fn pagination(prefix: &str, total: Option<usize>, offset: usize, busy: bool) {
    hidden(
        &format!("{prefix}-pagination"),
        total.map(|v| v <= 10).unwrap_or(true),
    );
    disabled(&format!("{prefix}-previous"), busy || offset == 0);
    disabled(
        &format!("{prefix}-next"),
        busy || total.map(|n| offset + 10 >= n).unwrap_or(true),
    );
    text(
        &format!("{prefix}-page"),
        &total
            .filter(|n| *n > 0)
            .map(|n| format!("{}–{} / {n}", offset + 1, n.min(offset + 10)))
            .unwrap_or_default(),
    );
}
fn render_online(s: &State, failed: bool) {
    let data = s.online.as_ref().unwrap_or(&Value::Null);
    let usable = !failed
        && data["available"] == true
        && data["stale"] == false
        && fresh(&data["updatedAt"], 45000.0)
        && data["miners"]
            .as_array()
            .map(|r| r.iter().all(Value::is_object))
            .unwrap_or(false)
        && data["total"].as_u64().is_some();
    let rows = data["miners"].as_array();
    html("online-miners-body", "");
    hidden(
        "online-table-wrap",
        !usable || rows.map(Vec::is_empty).unwrap_or(true),
    );
    text(
        "online-count",
        &if usable {
            number(&data["total"])
                + " "
                + if data["total"] == 1 {
                    tr("connexion", "connection")
                } else {
                    tr("connexions", "connections")
                }
        } else {
            "—".into()
        },
    );
    text(
        "online-updated",
        &if usable {
            tr("Relevé : ", "Checked: ").to_string()
                + &date(
                    &data["updatedAt"],
                    json!({"dateStyle":"short","timeStyle":"medium"}),
                )
        } else {
            String::new()
        },
    );
    pagination(
        "online",
        if usable {
            data["total"].as_u64().map(|v| v as usize)
        } else {
            None
        },
        s.online_offset,
        false,
    );
    if !usable {
        msg(
            "online-message",
            if data["status"] == "syncing" {
                tr(
                    "Le pool attend la synchronisation.",
                    "The pool is waiting to sync.",
                )
            } else {
                tr(
                    "Liste des connexions indisponible.",
                    "Connection list unavailable.",
                )
            },
            false,
        );
        return;
    }
    msg(
        "online-message",
        if data["total"] == 0 {
            tr("Aucun mineur connecté.", "No miners connected.")
        } else {
            ""
        },
        false,
    );
    let regex = Regex::new(if pool::bch() {
        r"(?i)(?:bitcoincash:)?[qp][a-z0-9]{41}"
    } else {
        r"(?:[13][1-9A-HJ-NP-Za-km-z]{25,34}|bc1[ac-hj-np-z02-9]{11,87})"
    })
    .unwrap();
    let mut output = String::new();
    for miner in rows.into_iter().flatten() {
        let worker = miner["worker"]
            .as_str()
            .map(|w| {
                regex
                    .replace_all(w, |c: &regex::Captures| masked(&c[0]))
                    .to_string()
            })
            .unwrap_or_else(|| tr("Sans nom", "Unnamed").into());
        output += &format!(
            "<tr>{}{}{}{}<td title=\"{}\">{}</td></tr>",
            device(&json!([miner["device"]]), true),
            cell(
                &masked(miner["addressMasked"].as_str().unwrap_or_default()),
                ""
            ),
            cell(&worker, ""),
            cell(&hash(&miner["hashrateHps"]["5m"]), ""),
            escape(&number(&miner["bestShare"])),
            escape(&compact(&miner["bestShare"]))
        );
    }
    html("online-miners-body", &output);
}
async fn api_get(path: &str, signal: Option<&web_sys::AbortSignal>) -> Result<Value, String> {
    let base = pool::config()["apiBaseUrl"]
        .as_str()
        .unwrap_or_default()
        .trim_end_matches('/')
        .to_string();
    let result = if let Some(signal) = signal {
        fetch_signal(&(base + path), signal).await
    } else {
        fetch(&(base + path), 8000).await
    }?;
    if result["apiVersion"] != "1" || (pool::bch() && result["coin"] != "bch") {
        Err("Invalid API schema".into())
    } else {
        Ok(result)
    }
}
fn load_online() {
    let mut offset = None;
    with(|s| {
        if s.online_busy {
            return;
        }
        s.online_busy = true;
        if s.online
            .as_ref()
            .map(|d| !fresh(&d["updatedAt"], 45000.0))
            .unwrap_or(false)
        {
            render_online(s, true);
        }
        pagination(
            "online",
            s.online
                .as_ref()
                .and_then(|v| v["total"].as_u64())
                .map(|n| n as usize),
            s.online_offset,
            true,
        );
        offset = Some(s.online_offset);
    });
    let Some(offset) = offset else {
        return;
    };
    run(async move {
        let mut result = api_get(&format!("/online?limit=10&offset={offset}"), None).await;
        if let Ok(d) = &result {
            if d["available"] == true
                && d["total"]
                    .as_u64()
                    .map(|n| n > 0 && offset >= n as usize)
                    .unwrap_or(false)
            {
                with(|s| s.online_offset = 0);
                result = api_get("/online?limit=10&offset=0", None).await;
            }
        }
        with(|s| {
            match result {
                Ok(data) => {
                    s.online = Some(data);
                    render_online(s, false);
                }
                Err(_) => render_online(s, true),
            }
            s.online_busy = false;
        });
    });
}
fn render_selected(s: &State) {
    let (Some(data), Some(address)) = (&s.data, &s.selected) else {
        return;
    };
    let miner = &data["miner"];
    let workers = &data["workers"];
    let present = !s.failed
        && miner["presenceAvailable"] == true
        && fresh(&miner["presenceUpdatedAt"], 45000.0);
    text("selected-miner-address", &masked(address));
    text("personal-hashrate", &hash(&miner["hashrateHps"]["5m"]));
    text(
        "personal-connections",
        &if present {
            number(&miner["connections"])
        } else {
            "—".into()
        },
    );
    text("personal-shares", &number(&miner["shares"]));
    text("personal-best", &compact(&best(miner)));
    attr("personal-best", "title", &number(&best(miner)));
    text(
        "miner-stats-updated",
        &(if miner["stale"] == true || !fresh(&miner["updatedAt"], 180000.0) {
            tr("Anciennes mesures · ", "Older readings · ")
        } else {
            tr("Relevé : ", "Updated: ")
        }
        .to_string()
            + &date(
                &miner["updatedAt"],
                json!({"dateStyle":"short","timeStyle":"medium"}),
            )),
    );
    let mut rows = String::new();
    for worker in workers["workers"].as_array().into_iter().flatten() {
        let name = worker["name"].as_str().unwrap_or_default();
        let suffix = name
            .strip_prefix(&format!("{address}."))
            .or_else(|| name.strip_prefix(&format!("{address}_")))
            .unwrap_or(if name == address {
                tr("Sans nom", "Unnamed")
            } else {
                "—"
            });
        let connected = if present && fresh(&worker["presenceUpdatedAt"], 45000.0) {
            worker["online"].as_bool()
        } else {
            None
        };
        rows += &format!(
            "<tr>{}{}{}{}<td title=\"{}\">{}</td></tr>",
            cell(suffix, ""),
            device(&worker["devices"], present && worker["online"] == true),
            cell(
                match connected {
                    Some(true) => tr("En ligne", "Online"),
                    Some(false) => tr("Hors ligne", "Offline"),
                    None => tr("Non confirmé", "Unconfirmed"),
                },
                if connected == Some(true) {
                    "worker-state online"
                } else {
                    "worker-state"
                }
            ),
            cell(&hash(&worker["hashrateHps"]["5m"]), ""),
            escape(&number(&best(worker))),
            escape(&compact(&best(worker)))
        );
    }
    if rows.is_empty() {
        rows = format!(
            "<tr><td class=\"empty-row\" colspan=\"5\">{}</td></tr>",
            tr("Aucun worker enregistré.", "No workers recorded.")
        );
    }
    html("personal-workers", &rows);
    pagination(
        "personal",
        workers["total"].as_u64().map(|v| v as usize),
        s.selected_offset,
        false,
    );
    hidden("miner-statistics", false);
}
fn load_selected(force: bool) {
    let mut request = None;
    with(|s| {
        if s.selected.is_none() || (s.busy && !force) {
            return;
        }
        if let Some(c) = s.controller.take() {
            c.abort();
        }
        let Ok(controller) = web_sys::AbortController::new() else {
            return;
        };
        s.controller = Some(controller.clone());
        s.generation += 1;
        s.busy = true;
        attr("miner-statistics", "aria-busy", "true");
        pagination(
            "personal",
            s.data
                .as_ref()
                .and_then(|d| d["workers"]["total"].as_u64())
                .map(|n| n as usize),
            s.selected_offset,
            true,
        );
        request = Some((
            s.selected.clone().unwrap(),
            s.selected_offset,
            s.generation,
            controller,
        ));
    });
    let Some((address, offset, generation, controller)) = request else {
        return;
    };
    let abort = controller.clone();
    let timer = later(8000, move || abort.abort());
    run(async move {
        let signal = controller.signal();
        let path = format!("/miners/{}", js_sys::encode_uri_component(&address));
        let result = async {
            let miner = api_get(&path, Some(&signal)).await?;
            let mut workers = api_get(
                &format!("{path}/workers?limit=10&offset={offset}"),
                Some(&signal),
            )
            .await?;
            if workers["total"]
                .as_u64()
                .map(|n| n > 0 && offset >= n as usize)
                .unwrap_or(false)
            {
                workers =
                    api_get(&format!("{path}/workers?limit=10&offset=0"), Some(&signal)).await?;
            }
            if miner["address"] != address
                || workers["address"] != address
                || !workers["workers"]
                    .as_array()
                    .map(|r| r.iter().all(|v| v["name"].is_string()))
                    .unwrap_or(false)
                || workers["total"].as_u64().is_none()
                || workers["offset"].as_u64().is_none()
            {
                return Err("Invalid miner response".to_string());
            }
            Ok(json!({"miner":miner,"workers":workers}))
        }
        .await;
        window().clear_timeout_with_handle(timer);
        with(|s| {
            if generation != s.generation {
                return;
            }
            s.busy = false;
            attr("miner-statistics", "aria-busy", "false");
            match result {
                Ok(data) => {
                    s.selected_offset = data["workers"]["offset"].as_u64().unwrap_or(0) as usize;
                    s.data = Some(data);
                    s.failed = false;
                    msg("miner-search-message", "", false);
                    render_selected(s);
                }
                Err(error) => {
                    controller.abort();
                    s.failed = true;
                    if let Some(data) = &s.data {
                        s.selected_offset =
                            data["workers"]["offset"].as_u64().unwrap_or(0) as usize;
                        render_selected(s);
                        msg("miner-search-message",tr("Actualisation indisponible. Les dernières mesures sont conservées.","Update unavailable. Previous readings are retained."),false);
                    } else {
                        msg(
                            "miner-search-message",
                            if error == "HTTP 404" {
                                tr("Aucune statistique pour cette adresse. BDLPool enregistre les données après une première share.","No statistics for this address. BDLPool records data after the first share.")
                            } else {
                                tr(
                                    "Statistiques indisponibles. Réessayez.",
                                    "Statistics unavailable. Try again.",
                                )
                            },
                            true,
                        );
                    }
                }
            }
        });
    });
}
fn refresh() {
    load_online();
    load_selected(false);
}
pub fn init() {
    if get("miner-search").is_none() {
        return;
    }
    on_id("miner-search", "submit", |event| {
        event.prevent_default();
        let entered = value("miner-lookup-address").trim().to_string();
        let address = address::canonical(&entered, pool::bch());
        with(|s| {
            if let Some(c) = s.controller.take() {
                c.abort();
            }
            s.generation += 1;
            s.busy = false;
            s.data = None;
            hidden("miner-statistics", true);
            s.selected = address;
            s.selected_offset = 0;
            if s.selected.is_none() {
                attr("miner-lookup-address", "aria-invalid", "true");
                msg(
                    "miner-search-message",
                    if pool::bch() {
                        tr(
                            "Vérifiez votre adresse CashAddr Bitcoin Cash.",
                            "Check your Bitcoin Cash CashAddr.",
                        )
                    } else {
                        tr(
                            "Vérifiez votre adresse Bitcoin.",
                            "Check your Bitcoin address.",
                        )
                    },
                    true,
                );
            } else {
                remove_attr("miner-lookup-address", "aria-invalid");
                msg("miner-search-message", tr("Chargement…", "Loading…"), false);
            }
        });
        load_selected(true);
    });
    on_id("miner-lookup-address", "input", |_| {
        remove_attr("miner-lookup-address", "aria-invalid")
    });
    for prefix in ["online", "personal"] {
        for (direction, delta) in [("previous", -10_isize), ("next", 10)] {
            click(&format!("{prefix}-{direction}"), move || {
                with(|s| {
                    let offset = if prefix == "online" {
                        &mut s.online_offset
                    } else {
                        &mut s.selected_offset
                    };
                    *offset = offset.saturating_add_signed(delta);
                });
                if prefix == "online" {
                    load_online();
                } else {
                    load_selected(true);
                }
            });
        }
    }
    click("refresh-miners", refresh);
    on(document().as_ref(), "visibilitychange", |_| {
        if !document().hidden() {
            with(|s| {
                render_online(s, false);
                render_selected(s);
            });
            refresh();
        }
    });
    interval(15000, || {
        if !document().hidden() {
            refresh();
        }
    });
    load_online();
}
