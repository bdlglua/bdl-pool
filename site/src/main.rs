mod html;

use std::{env, fs, io, path::Path};

const ORIGIN: &str = "https://bdl-pool.com";

#[derive(Clone, Copy, PartialEq)]
enum Language {
    Fr,
    En,
}

impl Language {
    fn index(self) -> usize {
        if self == Self::Fr {
            0
        } else {
            1
        }
    }
    fn code(self) -> &'static str {
        if self == Self::Fr {
            "fr"
        } else {
            "en"
        }
    }
    fn other(self) -> Self {
        if self == Self::Fr {
            Self::En
        } else {
            Self::Fr
        }
    }
}

struct Page {
    key: &'static str,
    slugs: [&'static str; 2],
    labels: [&'static str; 2],
    descriptions: [&'static str; 2],
    body: [&'static str; 2],
}

macro_rules! body {
    ($key:literal) => {
        [
            include_str!(concat!("../templates/pages/", $key, ".fr.html")),
            include_str!(concat!("../templates/pages/", $key, ".en.html")),
        ]
    };
}

const PAGES: &[Page] = &[
    Page { key: "home", slugs: ["", ""], labels: ["Accueil", "Home"], descriptions: [
        "Pool Bitcoin solo indépendant, connexions Stratum V1 et V2 et état du nœud Bitcoin.",
        "Independent Bitcoin solo pool, Stratum V1/V2 connections and Bitcoin node status."], body: body!("home") },
    Page { key: "statistics", slugs: ["statistiques", "statistics"], labels: ["Statistiques", "Statistics"], descriptions: [
        "Hashrate du pool, workers, meilleures shares et synchronisation du nœud Bitcoin.",
        "Pool hashrate, workers, best shares and Bitcoin node synchronisation."], body: body!("statistics") },
    Page { key: "miners", slugs: ["mineurs", "miners"], labels: ["Mineurs", "Miners"], descriptions: [
        "Recherchez les statistiques de votre adresse Bitcoin et consultez les mineurs en ligne.",
        "Look up your Bitcoin address statistics and view online miners."], body: body!("miners") },
    Page { key: "connect", slugs: ["connexion", "connect"], labels: ["Connexion", "Connection"], descriptions: [
        "Adresse, ports et clé publique Stratum V2 pour configurer votre mineur Bitcoin.",
        "Address, ports and Stratum V2 public key to configure your Bitcoin miner."], body: body!("connect") },
    Page { key: "faq", slugs: ["faq", "faq"], labels: ["FAQ", "FAQ"], descriptions: [
        "Questions sur la connexion, la disponibilité et les récompenses du pool Bitcoin.",
        "Questions about connecting, pool availability and Bitcoin block rewards."], body: body!("faq") },
    Page { key: "api", slugs: ["api", "api"], labels: ["API", "API"], descriptions: [
        "Statistiques du pool, des adresses Bitcoin et des workers via une API publique.",
        "Pool, Bitcoin address and worker statistics through the public API."], body: body!("api") },
];

fn route(page: &Page, lang: Language) -> String {
    let prefix = if lang == Language::Fr { "/" } else { "/en/" };
    let slug = page.slugs[lang.index()];
    if slug.is_empty() {
        prefix.into()
    } else {
        format!("{prefix}{slug}/")
    }
}

fn navigation(page: &Page, lang: Language) -> String {
    let mut output = String::new();
    for item in PAGES {
        let current = if item.key == page.key {
            " class=\"active\" aria-current=\"page\""
        } else {
            ""
        };
        output.push_str(&format!(
            "<a href=\"{}\"{current}>{}</a>",
            html::escape(&route(item, lang)),
            html::escape(item.labels[lang.index()])
        ));
    }
    output.push_str("<a href=\"https://miner.bdl-pool.com\">BDLMiner</a>");
    output
}

// A stable, non-security fingerprint for static asset cache invalidation.
fn fingerprint(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    format!("{hash:016x}")
}

fn asset(root: &Path, path: &str) -> io::Result<String> {
    Ok(format!(
        "/{path}?v={}",
        fingerprint(&fs::read(root.join(path))?)
    ))
}

fn styles(root: &Path) -> io::Result<String> {
    [
        "style",
        "miners",
        "api",
        "pool-theme",
        "pages",
        "pulse-theme",
    ]
    .iter()
    .map(|name| {
        asset(root, &format!("assets/{name}.css"))
            .map(|url| format!("<link rel=\"stylesheet\" href=\"{url}\">\n"))
    })
    .collect()
}

fn scripts(root: &Path, _page: &Page) -> io::Result<String> {
    let url = asset(root, "assets/wasm/boot.js")?;
    Ok(format!("<script type=\"module\" src=\"{url}\"></script>\n"))
}

fn build_browser(root: &Path) -> io::Result<()> {
    use std::process::Command;
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .args([
            "build",
            "--locked",
            "--release",
            "--target",
            "wasm32-unknown-unknown",
            "--manifest-path",
        ])
        .arg(root.join("site/browser/Cargo.toml"))
        .status()?;
    if !status.success() {
        return Err(io::Error::other("browser Rust compilation failed"));
    }
    let output = root.join("assets/wasm");
    fs::create_dir_all(&output)?;
    let status = Command::new("wasm-bindgen")
        .args(["--target", "web", "--no-typescript", "--out-dir"])
        .arg(&output)
        .arg(root.join("site/browser/target/wasm32-unknown-unknown/release/bdl_pool_browser.wasm"))
        .status()?;
    if !status.success() {
        return Err(io::Error::other("WebAssembly bindings failed"));
    }
    let module_hash = fingerprint(&fs::read(output.join("bdl_pool_browser.js"))?);
    let wasm_hash = fingerprint(&fs::read(output.join("bdl_pool_browser_bg.wasm"))?);
    fs::write(output.join("boot.js"), format!("// Generated WebAssembly loader. Application source: site/browser/src/*.rs\nimport init from './bdl_pool_browser.js?v={module_hash}';\nawait init({{module_or_path:new URL('./bdl_pool_browser_bg.wasm?v={wasm_hash}',import.meta.url)}});\n"))?;
    Ok(())
}

fn render(root: &Path, page: &Page, lang: Language) -> io::Result<String> {
    let index = lang.index();
    let title = if page.key == "home" {
        "BDL Pool".into()
    } else {
        format!("{} · BDL Pool", page.labels[index])
    };
    let legacy = if page.key == "home" {
        format!(
            r#"{{"dashboard":"{}","mineurs":"{}","connexion":"{}","faq":"{}"}}"#,
            route(&PAGES[1], lang),
            route(&PAGES[2], lang),
            route(&PAGES[3], lang),
            route(&PAGES[4], lang)
        )
    } else {
        "{}".into()
    };
    let layout = html::expand(
        if lang == Language::Fr {
            include_str!("../templates/layout-fr.html")
        } else {
            include_str!("../templates/layout-en.html")
        },
        &[
            ("icons", include_str!("../templates/icons.html").into()),
            ("navigation", navigation(page, lang)),
            ("language_url", html::escape(&route(page, lang.other()))),
            (
                "main_attrs",
                if page.key == "api" {
                    " class=\"api-docs wrap category-page\"".into()
                } else {
                    String::new()
                },
            ),
            ("content", page.body[index].into()),
        ],
    )?;
    html::expand(
        include_str!("../templates/document.html"),
        &[
            ("lang", lang.code().into()),
            ("page", html::escape(page.key)),
            ("home", html::escape(&route(&PAGES[0], lang))),
            ("legacy", html::escape(&legacy)),
            ("title", html::escape(&title)),
            ("description", html::escape(page.descriptions[index])),
            (
                "canonical",
                html::escape(&format!("{ORIGIN}{}", route(page, lang))),
            ),
            (
                "french_url",
                html::escape(&format!("{ORIGIN}{}", route(page, Language::Fr))),
            ),
            (
                "english_url",
                html::escape(&format!("{ORIGIN}{}", route(page, Language::En))),
            ),
            ("styles", styles(root)?),
            ("scripts", scripts(root, page)?),
            ("layout", layout),
        ],
    )
}

fn sitemap() -> String {
    let mut output = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\" xmlns:xhtml=\"http://www.w3.org/1999/xhtml\">\n");
    for lang in [Language::Fr, Language::En] {
        for page in PAGES {
            output.push_str(&format!("<url><loc>{ORIGIN}{}</loc>", route(page, lang)));
            for alternate in [Language::Fr, Language::En] {
                output.push_str(&format!(
                    "<xhtml:link rel=\"alternate\" hreflang=\"{}\" href=\"{ORIGIN}{}\"/>",
                    alternate.code(),
                    route(page, alternate)
                ));
            }
            output.push_str("</url>\n");
        }
    }
    output.push_str("</urlset>\n");
    output
}

fn copy_tree(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(io::Error::other("public assets must not contain symlinks"));
        }
        if kind.is_dir() {
            copy_tree(&entry.path(), &destination.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), destination.join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn build() -> io::Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    build_browser(root)?;
    let output = root.join("out");
    let mut documents = Vec::new();
    for lang in [Language::Fr, Language::En] {
        for page in PAGES {
            documents.push((
                format!("{}index.html", route(page, lang).trim_start_matches('/')),
                render(root, page, lang)?,
            ));
        }
    }
    // Only the generated out/ directory is replaced. Public resources are an
    // explicit allowlist: never stage the Python API or monitoring source tree.
    if output.exists() {
        fs::remove_dir_all(&output)?;
    }
    fs::create_dir_all(output.join("api"))?;
    copy_tree(&root.join("assets"), &output.join("assets"))?;
    copy_tree(&root.join("data"), &output.join("data"))?;
    for file in ["config.json", "CNAME", ".nojekyll", "api/openapi.json"] {
        fs::copy(root.join(file), output.join(file))?;
    }
    for (file, document) in &documents {
        let path = output.join(file);
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, document)?;
    }
    fs::write(output.join("sitemap.xml"), sitemap())?;
    fs::write(
        output.join("robots.txt"),
        format!("User-agent: *\nAllow: /\nSitemap: {ORIGIN}/sitemap.xml\n"),
    )?;
    fs::write(output.join("404.html"),"<!doctype html><html lang=\"fr\"><meta charset=\"utf-8\"><title>Page introuvable · BDL Pool</title><h1>Page introuvable</h1><a href=\"/\">BDL Pool</a></html>\n")?;
    println!(
        "Exported {} Rust-rendered pages to {}",
        documents.len(),
        output.display()
    );
    Ok(())
}

fn main() -> io::Result<()> {
    if env::args().len() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: bdl-pool-site",
        ));
    }
    build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn routes_and_metadata_cover_both_languages_without_a_framework() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let mut routes = HashSet::new();
        for lang in [Language::Fr, Language::En] {
            for page in PAGES {
                let url = route(page, lang);
                assert!(url.starts_with('/') && url.ends_with('/'));
                assert!(routes.insert(url.clone()));
                let html = render(root, page, lang).unwrap();
                assert!(html.contains(&format!("href=\"{ORIGIN}{url}\"")));
                assert!(html.contains(&format!("data-page=\"{}\"", page.key)));
                assert!(html.contains("<noscript>"));
                assert!(!html.contains("_next/"));
                assert!(!html.contains("self.__next_f"));
                assert!(html.contains("assets/wasm/boot.js"));
                assert!(!html.contains("chart.umd.min.js"));
            }
        }
        assert_eq!(routes.len(), 12);
        assert_eq!(route(&PAGES[3], Language::Fr), "/connexion/");
        assert_eq!(route(&PAGES[3], Language::En), "/en/connect/");
        assert_eq!(sitemap().matches("<url>").count(), 12);
    }

    #[test]
    fn fingerprints_are_stable_and_change_with_content() {
        assert_eq!(fingerprint(b"hello"), "a430d84680aabd0b");
        assert_ne!(fingerprint(b"a"), fingerprint(b"b"));
    }
}
