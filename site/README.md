# BDL Pool pages

The category contents live in `pages/fr/` and `pages/en/`. `layout.html`, `icons.html` and `tools/build_pages.py` provide the shared navigation, footer, language links and asset versions.

After changing a page or a public asset, run:

```bash
python3 tools/build_pages.py
python3 tools/build_pages.py --check
```

Commit the generated HTML and sitemap alongside the sources. GitHub Pages serves those static files from the repository root; no build service is required. The existing API and monitoring services are independent of this page builder.
