/* Progressive theme control for the Rust-rendered HTML. No hydration required. */
(() => {
  'use strict';
  const button = document.getElementById('theme-toggle');
  if (!button) return;
  const root = document.documentElement;
  const english = root.lang === 'en';
  function update() {
    const dark = root.dataset.theme !== 'light';
    const label = english
      ? (dark ? 'Switch to light theme' : 'Switch to dark theme')
      : (dark ? 'Activer le thème clair' : 'Activer le thème sombre');
    button.setAttribute('aria-pressed', String(dark));
    button.setAttribute('aria-label', label);
    button.title = label;
    document.querySelector('meta[name="theme-color"]')?.setAttribute('content', dark ? '#080f16' : '#f6f7fc');
  }
  button.addEventListener('click', () => {
    root.dataset.theme = root.dataset.theme === 'light' ? 'dark' : 'light';
    try { localStorage.setItem('bdl-pool:theme', root.dataset.theme); } catch {}
    update();
    document.dispatchEvent(new Event('bdl-theme-change'));
  });
  update();
})();
