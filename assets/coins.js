/* Network selection runs before any endpoint, cache or live-data consumer. */
(() => {
  'use strict';
  const coin = new URLSearchParams(location.search).get('coin') === 'bch' ? 'bch' : 'btc';
  const root = window.BDL_POOL_CONFIG || {};
  const config = coin === 'bch' ? {...root.coins?.bch, pollIntervalMs: root.pollIntervalMs} : {...root};
  const en = document.documentElement.lang === 'en';
  document.body.dataset.coin = coin;
  function address(value) {
    if (typeof value !== 'string') return null;
    if (coin === 'btc') {
      if (value.startsWith('BC1') && value === value.toUpperCase()) value = value.toLowerCase();
      return /^(?:[13][1-9A-HJ-NP-Za-km-z]{25,34}|bc1[ac-hj-np-z02-9]{11,87})$/.test(value) ? value : null;
    }
    if (value !== value.toLowerCase() && value !== value.toUpperCase()) return null;
    value = value.toLowerCase();
    if (!/^(?:bitcoincash:)?[qp][a-z0-9]{41}$/.test(value)) return null;
    const payload = value.split(':').at(-1);
    const alphabet = 'qpzry9x8gf2tvdw0s3jn54khce6mua7l';
    if ([...payload].some(c => !alphabet.includes(c))) return null;
    const values = [...'bitcoincash'].map(c => c.charCodeAt(0) & 31).concat(0, [...payload].map(c => alphabet.indexOf(c)));
    const generators = [0x98f2bc8e61n, 0x79b76d99e2n, 0xf33e5fb3c4n, 0xae2eabe2a8n, 0x1e4f43e470n];
    let checksum = 1n;
    for (const item of values) {
      const top = checksum >> 35n;
      checksum = ((checksum & 0x07ffffffffn) << 5n) ^ BigInt(item);
      generators.forEach((generator, bit) => { if ((top >> BigInt(bit)) & 1n) checksum ^= generator; });
    }
    let raw = 0n;
    for (const c of payload.slice(0, -8)) raw = (raw << 5n) | BigInt(alphabet.indexOf(c));
    if (checksum !== 1n || (raw & 3n) !== 0n || ![0n, 8n].includes(raw >> 162n)) return null;
    return 'bitcoincash:' + payload;
  }
  window.BDL_POOL_ACTIVE = {coin, config, address};
  const selector = document.getElementById('pool-coin');
  selector.value = coin;
  selector.addEventListener('change', () => {
    const url = new URL(location.href);
    url.searchParams.set('coin', selector.value);
    url.searchParams.delete('protocol');
    location.assign(url.href);
  });
  document.querySelectorAll('a[href]').forEach(link => {
    const url = new URL(link.href, location.href);
    if (url.origin !== location.origin || !url.pathname.endsWith('/') || coin !== 'bch') return;
    url.searchParams.set('coin', coin);
    link.href = url.href;
  });
  if (coin !== 'bch') return;
  // These pages use DOM-based data tools after hydration; update their initial
  // network labels before those tools start, leaving the network selector intact.
  const walker = document.createTreeWalker(document.getElementById('main'), NodeFilter.SHOW_TEXT);
  let node;
  while ((node = walker.nextNode())) {
    node.textContent = node.textContent
      .replace(/Bitcoin(?! Cash)/g, 'Bitcoin Cash').replace(/\bBTC\b/g, 'BCH')
      .replaceAll('VOTRE_ADRESSE_BTC', 'VOTRE_ADRESSE_BCH')
      .replaceAll('YOUR_BTC_ADDRESS', 'YOUR_BCH_ADDRESS')
      .replaceAll(root.apiBaseUrl, config.apiBaseUrl);
  }
  for (const id of ['btc-address', 'miner-lookup-address']) {
    const input = document.getElementById(id);
    if (input) input.placeholder = 'bitcoincash:q…';
  }
  const description = document.querySelector('.hero-description');
  if (description) description.textContent = en ? 'Bitcoin Cash solo mining · Stratum V1' : 'Minage Bitcoin Cash en solo · Stratum V1';
  document.querySelectorAll('a[href*="protocol=v2"]').forEach(link => { link.hidden = true; });
  document.querySelectorAll('[data-protocol="v2"]').forEach(button => { button.disabled = true; button.hidden = true; });
  const faq = document.getElementById('faq-connection');
  if (faq) faq.textContent = en ? 'Choose Bitcoin Cash on the Connection page to see its dedicated Stratum V1 endpoint. BCH uses a separate receiving address and network.' : 'Choisissez Bitcoin Cash sur la page Connexion pour voir son serveur Stratum V1 dédié. BCH utilise son propre réseau et une adresse de réception distincte.';
})();
