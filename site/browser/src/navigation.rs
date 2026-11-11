use crate::dom::*;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::JsCast;
use web_sys::{Element, KeyboardEvent};

pub fn init() {
    if let Some(body) = document().body() {
        if body.get_attribute("data-page").as_deref() == Some("home") {
            let redirects = serde_json::from_str::<serde_json::Value>(
                &body.get_attribute("data-legacy").unwrap_or_default(),
            )
            .unwrap_or_default();
            let redirect = move || {
                let hash = window().location().hash().unwrap_or_default();
                if let Some(target) = redirects[hash.trim_start_matches('#')].as_str() {
                    let _ = window().location().replace(&format!(
                        "{}{}",
                        target,
                        window().location().search().unwrap_or_default()
                    ));
                }
            };
            redirect();
            on(window().as_ref(), "hashchange", move |_| redirect());
        }
    }
    let theme = read_storage("bdl-pool:theme")
        .filter(|s| s == "light")
        .unwrap_or_else(|| "dark".into());
    if let Some(root) = document().document_element() {
        let _ = root.set_attribute("data-theme", &theme);
    }
    update_theme();
    click("theme-toggle", || {
        if let Some(root) = document().document_element() {
            let theme = if root.get_attribute("data-theme").as_deref() == Some("light") {
                "dark"
            } else {
                "light"
            };
            let _ = root.set_attribute("data-theme", theme);
            store("bdl-pool:theme", theme);
        }
        update_theme();
        dispatch("bdl-theme-change");
    });
    if let Some(toggle) = query(".menu-toggle") {
        on(toggle.as_ref(), "click", move |_| {
            let open = query(".menu-toggle")
                .and_then(|e| e.get_attribute("aria-expanded"))
                .as_deref()
                != Some("true");
            menu(open);
        });
    }
    for link in all("#main-nav a") {
        on(link.as_ref(), "click", |_| menu(false));
    }
    on(document().as_ref(), "click", |event| {
        if let (Some(header), Some(target)) = (
            query(".site-header"),
            event
                .target()
                .and_then(|e| e.dyn_into::<web_sys::Node>().ok()),
        ) {
            if !header.contains(Some(&target)) {
                menu(false);
            }
        }
    });
    on(document().as_ref(), "keydown", |event| {
        if event
            .dyn_ref::<KeyboardEvent>()
            .map(|e| e.key() == "Escape")
            .unwrap_or(false)
            && query(".menu-toggle")
                .and_then(|e| e.get_attribute("aria-expanded"))
                .as_deref()
                == Some("true")
        {
            menu(false);
            if let Some(el) = query(".menu-toggle") {
                let _ = call(&el, "focus", &[]);
            }
        }
    });
    floating();
    api();
}
fn update_theme() {
    let dark = document()
        .document_element()
        .and_then(|r| r.get_attribute("data-theme"))
        .as_deref()
        != Some("light");
    let label = if dark {
        tr("Activer le thème clair", "Switch to light theme")
    } else {
        tr("Activer le thème sombre", "Switch to dark theme")
    };
    attr(
        "theme-toggle",
        "aria-pressed",
        if dark { "true" } else { "false" },
    );
    attr("theme-toggle", "aria-label", label);
    attr("theme-toggle", "title", label);
    if let Some(el) = query("meta[name=theme-color]") {
        let _ = el.set_attribute("content", if dark { "#080f16" } else { "#f6f7fc" });
    }
}
fn menu(open: bool) {
    if let Some(toggle) = query(".menu-toggle") {
        let _ = toggle.set_attribute("aria-expanded", if open { "true" } else { "false" });
        let _ = toggle.set_attribute(
            "aria-label",
            if open {
                tr("Fermer le menu", "Close menu")
            } else {
                tr("Ouvrir le menu", "Open menu")
            },
        );
    }
    if let Some(nav) = get("main-nav") {
        class(&nav, "open", open);
    }
}
#[derive(Default)]
struct Scroll {
    previous: f64,
    direction: f64,
    distance: f64,
    pending: bool,
    focus_generation: u64,
}
fn position() -> f64 {
    let y = window().scroll_y().unwrap_or_default();
    let max = document()
        .document_element()
        .map(|e| f64::from(e.scroll_height()))
        .unwrap_or_default()
        - window()
            .inner_height()
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or_default();
    y.min(max.max(0.0)).max(0.0)
}
fn reveal(header: &Element, state: &Rc<RefCell<Scroll>>) {
    class(header, "nav-hidden", false);
    let mut s = state.borrow_mut();
    s.previous = position();
    s.direction = 0.0;
    s.distance = 0.0;
}
fn finish_focus(state: &Rc<RefCell<Scroll>>) {
    state.borrow_mut().focus_generation += 1;
    if let Some(root) = document().document_element() {
        class(&root, "nav-focusing", false);
    }
}
fn reveal_for_focus(header: &Element, state: &Rc<RefCell<Scroll>>) {
    if let Some(root) = document().document_element() {
        class(&root, "nav-focusing", true);
    }
    class(header, "nav-focus-reveal", true);
    reveal(header, state);
    header.get_bounding_client_rect();
    class(header, "nav-focus-reveal", false);
    let generation = {
        let mut state = state.borrow_mut();
        state.focus_generation += 1;
        state.focus_generation
    };
    let state = state.clone();
    frame(move || {
        if state.borrow().focus_generation == generation {
            if let Some(root) = document().document_element() {
                class(&root, "nav-focusing", false);
            }
        }
    });
}
pub fn floating() {
    let Some(header) = query(".site-header, #site-shell > nav, body > nav") else {
        return;
    };
    let state = Rc::new(RefCell::new(Scroll {
        previous: position(),
        ..Default::default()
    }));
    let h = header.clone();
    let s = state.clone();
    on(window().as_ref(), "scroll", move |_| {
        if s.borrow().pending {
            return;
        }
        s.borrow_mut().pending = true;
        let h = h.clone();
        let s = s.clone();
        frame(move || {
            let y = position();
            let delta = {
                let mut s = s.borrow_mut();
                s.pending = false;
                let d = y - s.previous;
                s.previous = y;
                d
            };
            let interacting = h
                .query_selector_all("[aria-expanded=true], details[open], :focus-visible")
                .ok()
                .map(|nodes| {
                    (0..nodes.length())
                        .filter_map(|i| nodes.item(i))
                        .filter_map(|n| n.dyn_into::<Element>().ok())
                        .any(|e| e.get_bounding_client_rect().width() > 0.0)
                })
                .unwrap_or(false);
            if y < 120.0 || interacting {
                reveal(&h, &s);
                return;
            }
            if delta == 0.0 {
                return;
            }
            let mut s = s.borrow_mut();
            let direction = delta.signum();
            if direction != s.direction {
                s.direction = direction;
                s.distance = 0.0;
            }
            s.distance += delta.abs();
            if s.distance >= if direction > 0.0 { 16.0 } else { 8.0 } {
                class(&h, "nav-hidden", direction > 0.0);
            }
        });
    });
    let h = header.clone();
    let s = state.clone();
    on(header.as_ref(), "focusin", move |_| {
        reveal_for_focus(&h, &s)
    });
    let h = header.clone();
    let s = state.clone();
    on(header.as_ref(), "toggle", move |_| reveal(&h, &s));
    let h = header.clone();
    let s = state.clone();
    on(document().as_ref(), "keydown", move |event| {
        if event
            .dyn_ref::<KeyboardEvent>()
            .map(|e| e.key() == "Tab")
            .unwrap_or(false)
        {
            reveal_for_focus(&h, &s);
        } else {
            finish_focus(&s);
        }
    });
    let s = state.clone();
    on(header.as_ref(), "click", move |_| finish_focus(&s));
    for name in ["resize", "pageshow"] {
        let h = header.clone();
        let s = state.clone();
        on(window().as_ref(), name, move |_| {
            finish_focus(&s);
            if let Some(toggle) = h.query_selector(".menu-toggle, #menu").ok().flatten() {
                if toggle.get_attribute("aria-expanded").as_deref() == Some("true")
                    && toggle.get_bounding_client_rect().width() == 0.0
                {
                    let _ = call(&toggle, "click", &[]);
                }
            }
            reveal(&h, &s);
        });
    }
}
fn api() {
    if get("api-health-test").is_none() {
        return;
    }
    for button in all("[data-copy]") {
        let target = button.get_attribute("data-copy").unwrap_or_default();
        on(button.as_ref(), "click", move |_| {
            let value = get(&target)
                .and_then(|e| e.text_content())
                .unwrap_or_default();
            run(async move {
                let ok = copy(&value).await;
                text(
                    "api-message",
                    if ok {
                        tr("Copié.", "Copied.")
                    } else {
                        tr(
                            "Sélectionnez le texte pour le copier.",
                            "Select the text to copy it.",
                        )
                    },
                );
                hidden("api-message", false);
            });
        });
    }
    click("api-health-test", || {
        run(async {
            disabled("api-health-test", true);
            hidden("api-health-result", true);
            text("api-message", tr("Test en cours…", "Testing…"));
            hidden("api-message", false);
            let base = get("api-base-url")
                .and_then(|e| e.text_content())
                .unwrap_or_default();
            let result = fetch(&format!("{}/health", base.trim()), 8000).await;
            match result {
                Ok(data)
                    if data["apiVersion"] == "1"
                        && data["status"] == "ok"
                        && (!crate::pool::bch() || data["coin"] == "bch") =>
                {
                    text(
                        "api-health-result",
                        &serde_json::to_string_pretty(&data).unwrap_or_default(),
                    );
                    hidden("api-health-result", false);
                    text(
                        "api-message",
                        tr("L’API répond (HTTP 200).", "The API responds (HTTP 200)."),
                    );
                }
                result => {
                    let message = result
                        .err()
                        .filter(|s| s.starts_with("HTTP "))
                        .unwrap_or_else(|| {
                            tr("API inaccessible en HTTPS.", "API unavailable over HTTPS.").into()
                        });
                    text("api-message", &message);
                }
            }
            disabled("api-health-test", false);
        })
    });
}
