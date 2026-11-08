(() => {
  'use strict';
  const english = document.documentElement.lang === 'en';
  const text = (fr, en) => english ? en : fr;
  const message = document.getElementById('api-message');
  function show(value) { message.textContent = value; message.hidden = false; }
  document.querySelectorAll('[data-copy]').forEach(button => button.addEventListener('click', async () => {
    try {
      await navigator.clipboard.writeText(document.getElementById(button.dataset.copy).textContent);
      show(text('Copié.', 'Copied.'));
    } catch {
      show(text('Sélectionnez le texte pour le copier.', 'Select the text to copy it.'));
    }
  }));
  const button = document.getElementById('api-health-test');
  button.addEventListener('click', async () => {
    button.disabled = true;
    const result = document.getElementById('api-health-result');
    result.hidden = true;
    show(text('Test en cours…', 'Testing…'));
    try {
      const base = document.getElementById('api-base-url').textContent.trim();
      const response = await fetch(base+'/health', {credentials:'omit', cache:'no-store', signal:AbortSignal.timeout(8000)});
      if (!response.ok) throw new Error('HTTP '+response.status);
      const data = await response.json();
      if (data.apiVersion !== '1' || data.status !== 'ok') throw new Error('invalid');
      result.textContent = JSON.stringify(data, null, 2);
      result.hidden = false;
      show(text('L’API répond (HTTP 200).', 'The API responds (HTTP 200).'));
    } catch (error) {
      show(/^HTTP \d+$/.test(error.message) ? error.message : text('API inaccessible en HTTPS.', 'API unavailable over HTTPS.'));
    } finally {
      button.disabled = false;
    }
  });
})();
