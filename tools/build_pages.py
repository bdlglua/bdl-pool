#!/usr/bin/env python3
"""Render the French/English GitHub Pages files from the common layout and page contents."""
from pathlib import Path
from string import Template
import argparse
import hashlib
import html
import json
import sys

ROOT = Path(__file__).resolve().parents[1]
PAGES = {
    'home': ('', '', 'Accueil', 'Home'),
    'statistics': ('statistiques', 'statistics', 'Statistiques', 'Statistics'),
    'miners': ('mineurs', 'miners', 'Mineurs', 'Miners'),
    'connect': ('connexion', 'connect', 'Connexion', 'Connection'),
    'faq': ('faq', 'faq', 'FAQ', 'FAQ'),
    'api': ('api', 'api', 'API', 'API'),
}
DESCRIPTIONS = {
    'home': ('Pool Bitcoin solo indépendant, connexions Stratum V1 et V2 et état du nœud Bitcoin.', 'Independent Bitcoin solo pool, Stratum V1/V2 connections and Bitcoin node status.'),
    'statistics': ('Hashrate du pool, workers, meilleures shares et synchronisation du nœud Bitcoin.', 'Pool hashrate, workers, best shares and Bitcoin node synchronisation.'),
    'miners': ('Recherchez les statistiques de votre adresse Bitcoin et consultez les mineurs en ligne.', 'Look up your Bitcoin address statistics and view online miners.'),
    'connect': ('Adresse, ports et clé publique Stratum V2 pour configurer votre mineur Bitcoin.', 'Address, ports and Stratum V2 public key to configure your Bitcoin miner.'),
    'faq': ('Questions sur la connexion, la disponibilité et les récompenses du pool Bitcoin.', 'Questions about connecting, pool availability and Bitcoin block rewards.'),
    'api': ('Statistiques du pool, des adresses Bitcoin et des workers via une API publique.', 'Pool, Bitcoin address and worker statistics through the public API.'),
}

def route(page, lang):
    slug = PAGES[page][lang == 'en']
    return ('/en/' if lang == 'en' else '/') + (slug + '/' if slug else '')

def asset(name):
    digest = hashlib.sha256((ROOT / name).read_bytes()).hexdigest()[:12]
    return '/' + name + '?v=' + digest

def render(page, lang):
    en = lang == 'en'
    esc = html.escape
    title = 'BDL Pool' if page == 'home' else PAGES[page][3 if en else 2] + ' · BDL Pool'
    home, alternate = route('home', lang), route(page, 'fr' if en else 'en')
    nav = ''.join('<a href="' + route(key, lang) + '"' + (' class="active" aria-current="page"' if key == page else '') + '>' + labels[3 if en else 2] + '</a>' for key, labels in PAGES.items())
    nav += '<a href="https://miner.bdl-pool.com">BDLMiner</a>'
    brand = '<a class="brand" href="' + home + '" aria-label="BDL Pool, ' + ('home' if en else 'accueil') + '"><img src="/assets/brand.svg" width="34" height="34" alt=""><span>BDL<span class="brand-light">Pool</span></span></a>'
    header = '<header class="site-header"><div class="header-inner wrap">' + brand + '<nav id="main-nav" class="main-nav" aria-label="' + ('Main navigation' if en else 'Navigation principale') + '">' + nav + '</nav><div class="header-actions"><a class="language-switch" href="' + alternate + '" lang="' + ('fr' if en else 'en') + '" hreflang="' + ('fr' if en else 'en') + '" aria-label="' + ('Passer en français' if en else 'Switch to English') + '">' + ('FR' if en else 'EN') + '</a><a class="header-github" href="https://github.com/bdlglua/bdl-pool" target="_blank" rel="noopener noreferrer">GitHub <svg class="icon" aria-hidden="true"><use href="#i-external"/></svg></a><button class="menu-toggle" type="button" aria-label="' + ('Open menu' if en else 'Ouvrir le menu') + '" aria-controls="main-nav" aria-expanded="false"><span></span><span></span><span></span></button></div></div></header>'
    footer = '<footer class="site-footer"><div class="footer-inner wrap"><nav class="footer-pages" aria-label="' + ('All pages' if en else 'Toutes les pages') + '">' + nav + '</nav><div class="footer-top">' + brand + '<p>' + ('Solo Bitcoin pool' if en else 'Pool Bitcoin solo') + '</p><a href="https://github.com/bdlglua/bdl-pool" target="_blank" rel="noopener noreferrer">GitHub <svg class="icon" aria-hidden="true"><use href="#i-external"/></svg></a></div><div class="footer-bottom"><span>© BDL Pool</span><a href="https://github.com/bdlglua" target="_blank" rel="noopener noreferrer">bdlglua</a></div></div></footer>'
    styles = ['style.css', 'pool-theme.css', 'pulse-theme.css', 'pages.css']
    if page == 'miners': styles.insert(1, 'miners.css')
    if page == 'api': styles.insert(1, 'api.css')
    scripts = ['assets/site.js', 'assets/navigation.js']
    if page != 'api': scripts.append('config.js')
    if page == 'statistics': scripts += ['assets/vendor/chart.umd.min.js', 'assets/pulse-chart.js']
    if page in ('home', 'statistics', 'connect', 'faq'): scripts.append('assets/app.js')
    if page == 'miners': scripts.append('assets/miners.js')
    if page == 'api': scripts.append('assets/api.js')
    legacy = {key: route(target, lang) for key, target in {'dashboard': 'statistics', 'mineurs': 'miners', 'connexion': 'connect', 'faq': 'faq'}.items()} if page == 'home' else {}
    data = dict(lang=lang, description=esc(DESCRIPTIONS[page][en]), title=esc(title), path=route(page, lang), fr_path=route(page, 'fr'), en_path=route(page, 'en'),
                theme_script=asset('assets/theme.js'), styles='\n'.join('  <link rel="stylesheet" href="' + asset('assets/' + name) + '">' for name in styles),
                scripts='\n'.join('  <script src="' + asset(name) + '" defer></script>' for name in scripts),
                page=page, home_path=home, legacy=esc(json.dumps(legacy, ensure_ascii=False)), icons=(ROOT/'site/icons.html').read_text(),
                skip='Skip to content' if en else 'Aller au contenu', header=header, footer=footer, main_class='api-docs wrap category-page' if page == 'api' else '',
                content=(ROOT/'site/pages'/lang/(page+'.html')).read_text(),
                noscript=('Enable JavaScript for live readings and miner settings. ' if en else 'Activez JavaScript pour les relevés et les réglages du mineur. ') + 'Stratum V1 : stratum.bdl-pool.com:39991 · Stratum V2 : stratum.bdl-pool.com:36809.')
    return Template((ROOT/'site/layout.html').read_text()).substitute(data)

def outputs():
    for lang in ('fr', 'en'):
        for page in PAGES:
            yield ROOT / route(page, lang).strip('/') / 'index.html', render(page, lang)
    locations = ''.join('<url><loc>https://bdl-pool.com' + route(page, lang) + '</loc></url>' for lang in ('fr', 'en') for page in PAGES)
    yield ROOT / 'sitemap.xml', '<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">' + locations + '</urlset>\n'

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='Fail if committed pages need rebuilding.')
    args = parser.parse_args()
    stale = []
    for path, content in outputs():
        if args.check:
            if not path.is_file() or path.read_text() != content: stale.append(str(path.relative_to(ROOT)))
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content)
    if stale:
        print('Run python3 tools/build_pages.py: ' + ', '.join(stale), file=sys.stderr)
        return 1
    print('Pages are current.' if args.check else 'Built 12 French/English pages and sitemap.xml.')
    return 0

if __name__ == '__main__':
    raise SystemExit(main())
