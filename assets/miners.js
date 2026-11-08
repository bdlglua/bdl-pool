(() => {
  'use strict';
  const en = document.documentElement.lang === 'en';
  const locale = en ? 'en-GB' : 'fr-FR';
  const t = (fr, english) => en ? english : fr;
  const $ = id => document.getElementById(id);
  const base = String(window.BDL_POOL_CONFIG?.apiBaseUrl || 'https://bdl-pool.duckdns.org/api/v1').replace(/\/$/, '');
  const addressPattern = /^(?:[13][1-9A-HJ-NP-Za-km-z]{25,34}|bc1[ac-hj-np-z02-9]{11,87})$/;
  const addressInText = /(?:[13][1-9A-HJ-NP-Za-km-z]{25,34}|bc1[ac-hj-np-z02-9]{11,87})/g;
  const pageSize = 10;
  let onlineOffset = 0, onlineData = null, onlineBusy = false;
  let selected = null, selectedData = null, selectedFailed = false, selectedBusy = false, generation = 0, controller = null;
  const finite = value => typeof value === 'number' && Number.isFinite(value) && value >= 0;
  const format = value => finite(value) ? new Intl.NumberFormat(locale, {maximumFractionDigits: 2}).format(value) : '—';
  const difficulty = value => finite(value) ? new Intl.NumberFormat(locale, {notation:'compact', maximumSignificantDigits:4}).format(value) : '—';
  function best(data) { const values = [data?.bestEver, data?.bestShare].filter(finite); return values.length ? Math.max(...values) : null; }
  function hash(value) {
    if (!finite(value)) return '—';
    const units = ['H/s','kH/s','MH/s','GH/s','TH/s','PH/s','EH/s']; let unit = 0;
    while (value >= 1000 && unit < units.length-1) { value /= 1000; unit++; }
    return format(value)+' '+units[unit];
  }
  function masked(value) {
    if (typeof value !== 'string' || !value) return '—';
    return value.includes('…') ? value.slice(0,40) : value.slice(0,6)+'…'+value.slice(-4);
  }
  function date(value) {
    const time = new Date(value);
    return value && Number.isFinite(time.getTime()) ? new Intl.DateTimeFormat(locale, {dateStyle:'short', timeStyle:'medium', timeZone:'Europe/Paris'}).format(time) : '—';
  }
  function fresh(value, seconds) {
    const age = Date.now()-Date.parse(value);
    return Number.isFinite(age) && age >= -60000 && age <= seconds*1000;
  }
  function message(id, value, error=false) { const el=$(id); el.textContent=value; el.hidden=!value; el.classList.toggle('error',error); }
  function cell(row, value, className) { const td=document.createElement('td');td.textContent=value;if(className)td.className=className;row.appendChild(td);return td; }
  function empty(body, text) { const row=document.createElement('tr');const td=cell(row,text,'empty-row');td.colSpan=5;body.replaceChildren(row); }
  function deviceCell(row, devices, known=true) {
    const td=cell(row,'','device-name');
    if (!known || !Array.isArray(devices) || devices.length===0) { td.textContent='—';return; }
    const labels=[...new Set(devices.slice(0,20).map(d=>typeof d?.model==='string' ? d.model : typeof d?.software==='string' ? d.software : t('Non identifié','Unidentified')))];
    td.textContent=labels.slice(0,3).join(', ')+(labels.length>3?' +'+(labels.length-3):'');
    const first=devices[0];
    const detail=[];
    if (!first?.model && first?.software) detail.push(t('Modèle non fourni','Model not provided'));
    if (first?.version && typeof first.version==='string') detail.push('v'+first.version);
    if (detail.length) { const small=document.createElement('small');small.textContent=detail.join(' · ');td.appendChild(small); }
  }
  function pagination(prefix, total, offset, busy=false) {
    const known=Number.isInteger(total) && total>=0;
    $(prefix+'-pagination').hidden=!known || total<=pageSize;
    $(prefix+'-previous').disabled=busy || offset===0;
    $(prefix+'-next').disabled=busy || !known || offset+pageSize>=total;
    $(prefix+'-page').textContent=known && total ? `${offset+1}–${Math.min(total,offset+pageSize)} / ${total}` : '';
  }
  async function get(path, signal) {
    const url=new URL(base+path,location.href);
    if (url.protocol!=='https:' && url.origin!==location.origin) throw new Error('Invalid API URL');
    const response=await fetch(url,{cache:'no-store',credentials:'omit',signal:signal || AbortSignal.timeout(8000)});
    if (!response.ok) { const error=new Error('API');error.status=response.status;throw error; }
    const data=await response.json();
    if (data.apiVersion!=='1') throw new Error('Invalid API schema');
    return data;
  }
  function renderOnline(data, failed=false) {
    const usable=!failed && data?.available===true && data.stale===false && fresh(data.updatedAt,45) && Array.isArray(data.miners) && data.miners.every(row=>row && typeof row==='object') && Number.isInteger(data.total) && data.total>=0;
    const body=$('online-miners-body');body.replaceChildren();
    $('online-table-wrap').hidden=!usable || data.miners.length===0;
    $('online-count').textContent=usable ? format(data.total)+' '+t(data.total===1?'connexion':'connexions',data.total===1?'connection':'connections') : '—';
    $('online-updated').textContent=usable ? t('Relevé : ','Checked: ')+date(data.updatedAt) : '';
    pagination('online',usable?data.total:null,onlineOffset);
    if (!usable) { message('online-message',data?.status==='syncing' ? t('Le pool attend la synchronisation.','The pool is waiting to sync.') : t('Liste des connexions indisponible.','Connection list unavailable.'));return; }
    message('online-message',data.total===0 ? t('Aucun mineur connecté.','No miners connected.') : '');
    for (const miner of data.miners) {
      const row=document.createElement('tr');deviceCell(row,[miner.device]);
      cell(row,masked(miner.addressMasked));
      cell(row,typeof miner.worker==='string' ? miner.worker.replace(addressInText,masked) : t('Sans nom','Unnamed'));
      cell(row,hash(miner.hashrateHps?.['5m']));const record=cell(row,difficulty(miner.bestShare));if(finite(miner.bestShare))record.title=format(miner.bestShare);
      body.appendChild(row);
    }
  }
  async function loadOnline() {
    if (onlineBusy) return;
    onlineBusy=true;
    if (onlineData && !fresh(onlineData.updatedAt,45)) renderOnline(onlineData,true);
    pagination('online',onlineData?.total,onlineOffset,true);
    try {
      let data=await get(`/online?limit=${pageSize}&offset=${onlineOffset}`);
      if (data.available && Number.isInteger(data.total) && data.total>0 && onlineOffset>=data.total) { onlineOffset=0;data=await get(`/online?limit=${pageSize}&offset=0`); }
      onlineData=data;renderOnline(data);
    } catch { renderOnline(onlineData,true); }
    finally { onlineBusy=false; }
  }
  function renderSelected() {
    if (!selectedData || !selected) return;
    const {miner,workers}=selectedData;
    const present=!selectedFailed && miner.presenceAvailable===true && fresh(miner.presenceUpdatedAt,45);
    $('selected-miner-address').textContent=masked(selected.address);
    $('personal-hashrate').textContent=hash(miner.hashrateHps?.['5m']);
    $('personal-connections').textContent=present?format(miner.connections):'—';
    $('personal-shares').textContent=format(miner.shares);
    $('personal-best').textContent=difficulty(best(miner));
    $('personal-best').title=finite(best(miner))?format(best(miner)):'';
    $('miner-stats-updated').textContent=(miner.stale || !fresh(miner.updatedAt,180) ? t('Anciennes mesures · ','Older readings · ') : t('Relevé : ','Updated: '))+date(miner.updatedAt);
    const body=$('personal-workers');body.replaceChildren();
    if (!workers.workers.length) empty(body,t('Aucun worker enregistré.','No workers recorded.'));
    for (const worker of workers.workers) {
      const row=document.createElement('tr');
      const name=String(worker.name || '');const prefix=selected.address;
      const suffix=name.startsWith(prefix+'.') || name.startsWith(prefix+'_') ? name.slice(prefix.length+1) : name===prefix ? t('Sans nom','Unnamed') : '—';
      cell(row,suffix);deviceCell(row,worker.devices,present && worker.online===true);
      const connected=present && fresh(worker.presenceUpdatedAt,45) ? worker.online : null;
      cell(row,connected===true?t('En ligne','Online'):connected===false?t('Hors ligne','Offline'):t('Non confirmé','Unconfirmed'),'worker-state'+(connected===true?' online':''));
      cell(row,hash(worker.hashrateHps?.['5m']));const record=cell(row,difficulty(best(worker)));if(finite(best(worker)))record.title=format(best(worker));body.appendChild(row);
    }
    pagination('personal',workers.total,selected.offset);
    $('miner-statistics').hidden=false;
  }
  async function loadSelected(force=false) {
    if (!selected || (selectedBusy && !force)) return;
    controller?.abort();const requestController=new AbortController();controller=requestController;
    const timeout=setTimeout(()=>requestController.abort(),8000);
    const current=++generation,address=selected.address;selectedBusy=true;
    $('miner-statistics').setAttribute('aria-busy','true');
    pagination('personal',selectedData?.workers.total,selected.offset,true);
    try {
      const path='/miners/'+encodeURIComponent(address);
      const signal=requestController.signal;
      let [miner,workers]=await Promise.all([get(path,signal),get(path+`/workers?limit=${pageSize}&offset=${selected.offset}`,signal)]);
      if (current!==generation) return;
      if (workers.total>0 && selected.offset>=workers.total) { selected.offset=0;workers=await get(path+`/workers?limit=${pageSize}&offset=0`,signal); }
      if (current!==generation) return;
      if (miner.address!==address || workers.address!==address || !Array.isArray(workers.workers) || !workers.workers.every(row=>row && typeof row.name==='string') || !Number.isInteger(workers.total) || workers.total<0 || !Number.isInteger(workers.offset) || workers.offset<0) throw new Error('Invalid miner response');
      selected.offset=workers.offset;
      selectedData={miner,workers};selectedFailed=false;message('miner-search-message','');renderSelected();
    } catch (error) {
      if (current!==generation) return;
      requestController.abort();
      selectedFailed=true;
      if (selectedData) { selected.offset=selectedData.workers.offset;renderSelected();message('miner-search-message',t('Actualisation indisponible. Les dernières mesures sont conservées.','Update unavailable. Previous readings are retained.')); }
      else message('miner-search-message',error.status===404 ? t('Aucune statistique pour cette adresse. BDLPool enregistre les données après une première share.','No statistics for this address. BDLPool records data after the first share.') : t('Statistiques indisponibles. Réessayez.','Statistics unavailable. Try again.'),true);
    } finally {
      clearTimeout(timeout);
      if(current===generation){selectedBusy=false;$('miner-statistics').setAttribute('aria-busy','false');}
    }
  }
  $('miner-search').addEventListener('submit',event=>{
    event.preventDefault();controller?.abort();generation++;selectedBusy=false;selectedData=null;$('miner-statistics').hidden=true;
    const address=$('miner-lookup-address').value.trim();
    if (!addressPattern.test(address)) { selected=null;$('miner-lookup-address').setAttribute('aria-invalid','true');message('miner-search-message',t('Vérifiez votre adresse Bitcoin.','Check your Bitcoin address.'),true);return; }
    $('miner-lookup-address').removeAttribute('aria-invalid');selected={address,offset:0};message('miner-search-message',t('Chargement…','Loading…'));loadSelected(true);
  });
  $('miner-lookup-address').addEventListener('input',()=>$('miner-lookup-address').removeAttribute('aria-invalid'));
  for(const [prefix,action] of [['online',()=>loadOnline()],['personal',()=>loadSelected(true)]]){
    for(const [direction,delta] of [['previous',-pageSize],['next',pageSize]])$(prefix+'-'+direction).addEventListener('click',()=>{
      if(prefix==='online')onlineOffset=Math.max(0,onlineOffset+delta);else if(selected)selected.offset=Math.max(0,selected.offset+delta);
      action();
    });
  }
  async function refresh() { await Promise.all([loadOnline(),loadSelected()]); }
  $('refresh-miners').addEventListener('click',async()=>{ $('refresh-miners').disabled=true;try{await refresh();}finally{$('refresh-miners').disabled=false;} });
  document.addEventListener('visibilitychange',()=>{if(!document.hidden){if(onlineData)renderOnline(onlineData);if(selectedData)renderSelected();refresh();}});
  setInterval(()=>{if(!document.hidden)refresh();},15000);
  loadOnline();
})();
