/* Shared navigation for every pool page; category tools load separately. */
(() => {
  'use strict';
  const {page, legacy} = document.body.dataset;
  if (page === 'home') {
    const redirects = new Map(Object.entries(JSON.parse(legacy || '{}')));
    function redirectLegacy() {
      const target = redirects.get(location.hash.slice(1));
      if (!target) return false;
      window.BDL_POOL_REDIRECTING = true;
      location.replace(target + location.search);
      return true;
    }
    if (redirectLegacy()) return;
    window.addEventListener('hashchange', redirectLegacy);
  }
  const toggle = document.querySelector('.menu-toggle'), nav = document.getElementById('main-nav');
  const en = document.documentElement.lang === 'en';
  function setMenu(open) {
    toggle.setAttribute('aria-expanded', String(open));
    toggle.setAttribute('aria-label', en ? (open ? 'Close menu' : 'Open menu') : (open ? 'Fermer le menu' : 'Ouvrir le menu'));
    nav.classList.toggle('open', open);
  }
  toggle.addEventListener('click', () => setMenu(toggle.getAttribute('aria-expanded') !== 'true'));
  nav.querySelectorAll('a').forEach(link => link.addEventListener('click', () => setMenu(false)));
  document.addEventListener('click', event => {
    if (toggle.getAttribute('aria-expanded') === 'true' && !toggle.closest('.site-header').contains(event.target)) setMenu(false);
  });
  document.addEventListener('keydown', event => {
    if (event.key === 'Escape' && toggle.getAttribute('aria-expanded') === 'true') {
      setMenu(false);
      toggle.focus({preventScroll: true});
    }
  });
})();
