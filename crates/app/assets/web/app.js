/* Batiie – the little bit of script around htmx:
   dialogs, menus, keyboard, column resizing, drag & drop. Everything that
   changes data is an htmx request; this file only does what a browser needs. */
(() => {
  'use strict';
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
  const boot = (() => { try { return JSON.parse($('#batiie-boot').textContent); } catch (_) { return {}; } })();
  const ajax = (method, url, values, opts = {}) =>
    htmx.ajax(method, url, Object.assign({ source: '#list', swap: 'none', values: values || {} }, opts));
  const dialogGet = (url) => htmx.ajax('GET', url, { source: '#list', target: '#modal-host', swap: 'beforeend' });
  const inField = (t) => t && (t.closest('input,textarea,select,[contenteditable=true]') !== null);
  const modalOpen = () => $('dialog[open][data-modal]') !== null;

  /* ------------------------------------------------------------ dialogs */
  const host = $('#modal-host');
  const closedByServer = new WeakSet();
  const show = (d) => {
    if (d.dataset.shown) return;
    const twin = $$('dialog[data-modal]', host).find((o) => o !== d && o.id === d.id && o.dataset.keep !== undefined);
    if (twin) { d.remove(); const f = twin.querySelector('input.in'); if (f) f.focus(); return; }
    d.dataset.shown = '1';
    // the same window twice: the older one goes
    $$('dialog[data-modal]', host).forEach((o) => { if (o !== d && o.id === d.id) { closedByServer.add(o); o.close(); o.remove(); } });
    d.addEventListener('close', () => {
      const url = d.dataset.onclose;
      if (url && !closedByServer.has(d)) htmx.ajax('POST', url, { source: '#list', swap: 'none' });
      d.remove();
    });
    d.showModal();
    const first = d.querySelector('[autofocus]') || d.querySelector('input.in:not([type=hidden]):not([readonly]),textarea.in,select.in');
    if (first) { first.focus({ preventScroll: true }); if (first.select && first.tagName === 'INPUT' && first.type === 'text' && first.dataset.noselect === undefined) first.select(); }
  };
  new MutationObserver((muts) => {
    for (const m of muts) m.addedNodes.forEach((n) => {
      if (n.nodeType === 1 && n.matches && n.matches('dialog[data-modal]')) show(n);
    });
  }).observe(host, { childList: true });
  const closeDialog = (id) => {
    const d = document.getElementById(id);
    if (d && d.tagName === 'DIALOG') { closedByServer.add(d); d.close(); d.remove(); }
  };
  document.body.addEventListener('dlgClose', (e) => closeDialog(typeof e.detail === 'string' ? e.detail : (e.detail && e.detail.value)));
  document.addEventListener('click', (e) => {
    const x = e.target.closest('[data-close]');
    if (x) { const d = x.closest('dialog,.float'); if (d) { d.tagName === 'DIALOG' ? d.close() : closeFloat(d); } }
    const f = e.target.closest('[data-focus]');
    if (f) { const t = $(f.dataset.focus); closeMenus(); if (t) t.focus(); }
  });
  // clicking the dark area around a dialog does not close it (a slip would lose typing)

  /* ------------------------------------------------------------ alerts and toasts */
  const alertBox = (text) => {
    const d = document.createElement('dialog');
    d.className = 'dlg small'; d.dataset.modal = ''; d.id = 'alert-' + Date.now();
    d.innerHTML = '<div class="dlg-head"><span class="dlg-title"></span></div><div class="dlg-body ask"><div class="ask-t sel-text"></div></div><div class="dlg-foot"><button type="button" class="btn primary" data-close autofocus></button></div>';
    d.querySelector('.dlg-title').textContent = boot.notice || 'Batiie';
    const t = d.querySelector('.ask-t');
    String(text).split('\n').forEach((l) => { const p = document.createElement('p'); p.textContent = l; if (!l) p.innerHTML = '&nbsp;'; t.appendChild(p); });
    d.querySelector('.btn').textContent = boot.ok || 'OK';
    host.appendChild(d);
  };
  const toast = (text) => {
    const el = document.createElement('div');
    el.className = 'toast'; el.textContent = text;
    $('#toasts').appendChild(el);
    setTimeout(() => { el.classList.add('out'); setTimeout(() => el.remove(), 300); }, 3200);
  };

  /* ------------------------------------------------------------ live connection */
  document.body.addEventListener('htmx:sseBeforeMessage', (e) => {
    const type = String(e.detail.type || '').replace(/^batiie-/, ''), data = e.detail.data;
    if (type === 'ui') return;
    e.preventDefault();
    if (type === 'open') {
      if (data === 'reload') location.reload();
      else dialogGet(data);
    } else if (type === 'alert') alertBox(data);
    else if (type === 'toast') toast(data);
    else if (type === 'close') closeDialog(data);
  });
  // the look: theme, text size, language
  document.body.addEventListener('htmx:oobAfterSwap', (e) => {
    const old = e.detail.target || e.target;
    const id = old && old.id;
    // the element that was swapped in (the detail may still point at the one that went)
    const t = id ? document.getElementById(id) : null;
    if (t && id === 'look') applyLook(t.dataset.look);
    if (t && id === 'title' && t.dataset.title) document.title = t.dataset.title;
  });
  const applyLook = (s) => {
    if (!s) return;
    const [theme, fs, family, lang, bg, bgs] = s.split('|');
    const h = document.documentElement;
    h.dataset.theme = theme;
    h.style.setProperty('--fs', String(Math.min(2, Math.max(0.7, (+fs || 100) / 100))));
    h.style.setProperty('--bgs', String(Math.min(0.8, (+bgs || 45) / 100)));
    if (family) h.style.setProperty('--font', "'" + family.replace(/[^\w \-]/g, '') + "',system-ui,sans-serif"); else h.style.removeProperty('--font');
    if (lang && lang !== h.lang) { location.reload(); return; }
    const img = $('.bgimg');
    if (bg && !img) { const d = document.createElement('div'); d.className = 'bgimg'; d.style.backgroundImage = 'url(/bg?' + Date.now() + ')'; document.body.prepend(d); }
    else if (!bg && img) img.remove();
  };

  /* ------------------------------------------------------------ menus */
  const drop = $('#menu-drop'), ctx = $('#ctx');
  // hidden, not emptied: a request started from a menu item still needs that element to
  // exist when its answer arrives (htmx looks up the target from it)
  const closeMenus = () => { drop.hidden = true; ctx.hidden = true; $$('.mt.open').forEach((b) => b.classList.remove('open')); };
  let openMenu = null;
  const place = (menu, x, y) => {
    const m = menu.getBoundingClientRect(), w = innerWidth, h = innerHeight;
    if (x + m.width > w - 6) x = Math.max(6, w - m.width - 6);
    if (y + m.height > h - 6) y = Math.max(6, h - m.height - 6);
    menu.style.left = x + 'px'; menu.style.top = y + 'px';
  };
  document.body.addEventListener('htmx:afterSwap', (e) => {
    const t = e.detail.target;
    if (t === drop) drop.hidden = false;
    if (t === ctx) ctx.hidden = false;
    if (t === drop && openMenu) {
      const menu = drop.firstElementChild; if (!menu) return;
      const r = openMenu.getBoundingClientRect();
      menu.style.left = '0px'; menu.style.top = '0px';
      const rtl = document.dir === 'rtl' || getComputedStyle(document.documentElement).direction === 'rtl';
      place(menu, rtl ? r.right - menu.getBoundingClientRect().width : r.left, r.bottom + 2);
      $$('.has-sub', menu).forEach(flipCheck);
    }
    if (t === ctx) { const menu = ctx.firstElementChild; if (menu) { place(menu, +menu.dataset.x, +menu.dataset.y); $$('.has-sub', menu).forEach(flipCheck); } }
  });
  const flipCheck = (li) => {
    li.addEventListener('mouseenter', () => {
      const sm = li.querySelector('.submenu'); if (!sm) return;
      li.classList.remove('flip', 'up');
      const r = li.getBoundingClientRect(), w = sm.offsetWidth || 220, hh = sm.scrollHeight || 200;
      const rtl = getComputedStyle(document.documentElement).direction === 'rtl';
      if (rtl ? r.left - w < 4 : r.right + w > innerWidth - 4) li.classList.add('flip');
      if (r.top + hh > innerHeight - 6) li.classList.add('up');
    });
  };
  document.addEventListener('click', (e) => {
    const mt = e.target.closest('.mt');
    if (mt) {
      if (openMenu === mt) { closeMenus(); openMenu = null; e.stopPropagation(); e.preventDefault(); return; }
      closeMenus(); openMenu = mt; mt.classList.add('open');
      return;
    }
    if (e.target.closest('.menu .mi.has-sub')) return;
    if (e.target.closest('.menu .mi')) { setTimeout(() => { closeMenus(); openMenu = null; }, 0); return; }
    if (!e.target.closest('.menu')) { closeMenus(); openMenu = null; }
  }, true);
  document.addEventListener('mouseover', (e) => {
    const mt = e.target.closest('.mt');
    if (mt && openMenu && mt !== openMenu) mt.click();
  });
  addEventListener('blur', () => { closeMenus(); openMenu = null; });

  /* ------------------------------------------------------------ the list */
  const list = $('#list');
  const rowOf = (e) => e.target.closest('.row');
  document.addEventListener('click', (e) => {
    // the small buttons that show when the pointer is over a line
    const q = e.target.closest('.rq');
    if (q) { e.stopPropagation(); e.preventDefault(); ajax('POST', q.dataset.act, { id: q.closest('.row').dataset.id }); return; }
    const row = rowOf(e);
    if (row && list.contains(row)) ajax('POST', '/sel/click', { id: row.dataset.id, ctrl: e.ctrlKey || e.metaKey ? 1 : 0, shift: e.shiftKey ? 1 : 0 });
  });
  document.addEventListener('dblclick', (e) => {
    const row = rowOf(e);
    if (row && list.contains(row)) ajax('POST', '/act/activate', { id: row.dataset.id });
  });
  document.addEventListener('contextmenu', (e) => {
    const row = rowOf(e);
    if (row && list.contains(row)) {
      e.preventDefault(); closeMenus(); openMenu = null;
      htmx.ajax('POST', '/ctx', { source: '#list', target: '#ctx', swap: 'innerHTML', values: { id: row.dataset.id, x: e.clientX, y: e.clientY } });
    } else if (!inField(e.target)) e.preventDefault();
  });

  /* ------------------------------------------------------------ keyboard */
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') { if (!drop.hidden && drop.firstChild || !ctx.hidden && ctx.firstChild) { closeMenus(); openMenu = null; e.preventDefault(); } return; }
    if (modalOpen()) return;
    const typing = inField(e.target);
    const mod = e.ctrlKey || e.metaKey;
    const k = e.key;
    if (mod && !e.altKey) {
      if (k === 'n' || k === 'N') { e.preventDefault(); dialogGet('/dlg/add'); return; }
      if (k === 'f' || k === 'F') { e.preventDefault(); dialogGet('/dlg/find'); return; }
      if (typing) return;
      if (k === 'v' || k === 'V') { e.preventDefault(); ajax('POST', '/act/paste'); return; }
      if (k === 'a' || k === 'A') { e.preventDefault(); ajax('POST', '/sel/all'); return; }
      if (k === ',') { e.preventDefault(); dialogGet('/dlg/settings'); return; }
      return;
    }
    if (typing) return;
    if (k === 'F1' || (k === '?' && !mod)) { e.preventDefault(); dialogGet('/dlg/shortcuts'); return; }
    if (k === 'F3') { e.preventDefault(); ajax('POST', '/act/findnext'); return; }
    if (k === 'Delete') { e.preventDefault(); ajax('POST', '/act/delete'); return; }
    if (k === 'Enter') { e.preventDefault(); ajax('POST', '/act/activate', {}); return; }
    const map = { ArrowUp: 'up', ArrowDown: 'down', Home: 'home', End: 'end', PageUp: 'pgup', PageDown: 'pgdn' };
    if (map[k]) {
      e.preventDefault();
      ajax('POST', '/sel/key', { k: map[k], shift: e.shiftKey ? 1 : 0 });
      requestAnimationFrame(() => setTimeout(() => { const f = $('.row.foc'); if (f) f.scrollIntoView({ block: 'nearest' }); }, 60));
    }
  });

  /* ------------------------------------------------------------ columns: resize and move */
  let drag = null;
  const setCols = (key, w) => {
    const head = $('#lhead'); if (!head) return;
    const keys = $$('.hc', head).map((c) => c.dataset.key);
    const cols = getComputedStyle(head).gridTemplateColumns.split(' ').map((v) => parseFloat(v));
    const i = keys.indexOf(key); if (i < 0) return;
    cols[i] = w;
    const val = cols.map((v) => Math.round(v) + 'px').join(' ');
    const total = cols.reduce((a, b) => a + b, 0);
    [head, list].forEach((el) => { el.style.setProperty('--cols', val); el.style.minWidth = total + 'px'; });
  };
  document.addEventListener('pointerdown', (e) => {
    const rs = e.target.closest('.rs');
    if (rs) {
      e.preventDefault(); e.stopPropagation();
      const cell = rs.parentElement;
      drag = { type: 'col', key: rs.dataset.key, x: e.clientX, w: cell.getBoundingClientRect().width, rtl: getComputedStyle(document.documentElement).direction === 'rtl', el: rs };
      rs.classList.add('drag'); rs.setPointerCapture(e.pointerId);
      return;
    }
    const sp = e.target.closest('#split');
    if (sp) {
      e.preventDefault();
      drag = { type: 'side', x: e.clientX, w: $('.side').getBoundingClientRect().width, rtl: getComputedStyle(document.documentElement).direction === 'rtl', el: sp };
      sp.classList.add('drag'); sp.setPointerCapture(e.pointerId);
    }
  }, true);
  document.addEventListener('pointermove', (e) => {
    if (!drag) return;
    const dx = (e.clientX - drag.x) * (drag.rtl ? -1 : 1);
    if (drag.type === 'col') setCols(drag.key, Math.max(36, Math.min(2000, drag.w + dx)));
    else $('.main').style.setProperty('--side-w', Math.max(150, Math.min(520, drag.w + dx)) + 'px');
  });
  document.addEventListener('pointerup', () => {
    if (!drag) return;
    const d = drag; drag = null; d.el.classList.remove('drag');
    if (d.type === 'col') {
      const head = $('#lhead'), keys = $$('.hc', head).map((c) => c.dataset.key);
      const w = parseFloat(getComputedStyle(head).gridTemplateColumns.split(' ')[keys.indexOf(d.key)]);
      ajax('POST', '/cols/width', { key: d.key, w: Math.round(w) });
    } else { try { localStorage.setItem('side', $('.main').style.getPropertyValue('--side-w')); } catch (_) {} }
  });
  document.addEventListener('click', (e) => { if (e.target.closest('.rs')) { e.stopPropagation(); e.preventDefault(); } }, true);
  try { const s = localStorage.getItem('side'); if (s) $('.main').style.setProperty('--side-w', s); } catch (_) {}
  // move a column by dragging its title
  let moving = null;
  document.addEventListener('dragstart', (e) => {
    const hc = e.target.closest && e.target.closest('.hc');
    if (hc) { moving = hc.dataset.key; e.dataTransfer.setData('application/x-batiie-col', moving); e.dataTransfer.effectAllowed = 'move'; }
  });
  document.addEventListener('dragover', (e) => {
    if (!moving) return;
    const hc = e.target.closest('.hc'); $$('.hc.dragover').forEach((c) => c.classList.remove('dragover'));
    if (hc && hc.dataset.key !== moving) { e.preventDefault(); hc.classList.add('dragover'); }
  });
  document.addEventListener('drop', (e) => {
    if (!moving) return;
    const hc = e.target.closest('.hc');
    if (hc && hc.dataset.key !== moving) { e.preventDefault(); e.stopPropagation(); ajax('POST', '/cols/move', { key: moving, before: hc.dataset.key }); }
    $$('.hc.dragover').forEach((c) => c.classList.remove('dragover')); moving = null;
  }, true);
  document.addEventListener('dragend', () => { moving = null; $$('.hc.dragover').forEach((c) => c.classList.remove('dragover')); });

  /* ------------------------------------------------------------ drag & drop onto the window */
  let depth = 0;
  const hasStuff = (e) => e.dataTransfer && Array.from(e.dataTransfer.types || []).some((t) => t === 'Files' || t === 'text/uri-list' || t === 'text/plain');
  addEventListener('dragenter', (e) => { if (hasStuff(e) && !moving) { depth++; document.body.classList.add('dragging'); } });
  addEventListener('dragleave', () => { if (depth > 0 && --depth === 0) document.body.classList.remove('dragging'); });
  addEventListener('dragover', (e) => { if (hasStuff(e) && !moving) e.preventDefault(); });
  addEventListener('drop', async (e) => {
    document.body.classList.remove('dragging'); depth = 0;
    if (moving || !e.dataTransfer) return;
    e.preventDefault();
    const files = Array.from(e.dataTransfer.files || []);
    if (files.length) {
      const tor = files.filter((f) => /\.torrent$/i.test(f.name));
      if (tor.length) {
        const f = tor[0];
        const r = await fetch('/upload/torrent', { method: 'POST', headers: { 'X-Batiie': '1', 'X-Filename': encodeURIComponent(f.name) }, body: f });
        if (r.ok) dialogGet('/dlg/torrent?upload=1');
        if (tor.length > 1) toast(tor.length + ' ✕ .torrent');
      } else toast(boot.onlyTorrent || 'Only .torrent files can be dropped on the window');
      return;
    }
    const text = e.dataTransfer.getData('text/uri-list') || e.dataTransfer.getData('text/plain');
    if (text) ajax('POST', '/drop', { text });
  });

  /* ------------------------------------------------------------ small helpers for dialogs */
  // a text box that should check itself after a pause (Add window)
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && e.target.matches('dialog input.in[type=text],dialog input.in[type=password],dialog input.in.num')) {
      const f = e.target.closest('form'); if (f && !e.target.closest('[data-noenter]')) { e.preventDefault(); const b = f.querySelector('[data-enter]') || f.querySelector('button.primary[type=submit]'); if (b) b.click(); }
    }
  });
  // all / none in lists of check boxes
  document.addEventListener('click', (e) => {
    const b = e.target.closest('[data-checkall]'); if (!b) return;
    const scope = b.closest('dialog,form') || document;
    $$(b.dataset.checkall, scope).forEach((c) => { c.checked = b.dataset.state !== '0'; c.dispatchEvent(new Event('change', { bubbles: true })); });
  });
  // fields that follow another one (a "use this" check box that enables inputs)
  const syncDeps = (root) => $$('[data-enable]', root || document).forEach((el) => {
    const src = $(el.dataset.enable); if (!src) return;
    const on = src.type === 'checkbox' ? src.checked : !!src.value;
    $$('input,select,textarea,button', el).forEach((c) => { if (c !== src) c.disabled = !on; });
    el.classList.toggle('disabled', !on);
  });
  document.addEventListener('change', (e) => { if (e.target.closest('dialog')) syncDeps(e.target.closest('dialog')); });
  document.body.addEventListener('htmx:afterSettle', (e) => { const d = e.target.closest && e.target.closest('dialog'); if (d) syncDeps(d); });
  new MutationObserver((m) => m.forEach((r) => r.addedNodes.forEach((n) => { if (n.nodeType === 1 && n.matches('dialog')) setTimeout(() => syncDeps(n), 0); }))).observe(host, { childList: true });

  // links in dialogs (About, Update): the real browser
  document.addEventListener('click', (e) => {
    const a = e.target.closest('a[data-ext]');
    if (a) { e.preventDefault(); ajax('POST', '/act/openurl', { url: a.dataset.ext }); }
  });

  // "Remove" next to a picture box: empty it and tell htmx it changed
  document.addEventListener('click', (e) => {
    const c = e.target.closest('[data-clear]'); if (!c) return;
    const i = $(c.dataset.clear); if (i) { i.value = ''; htmx.trigger(i, 'change'); }
  });
  // a box that was filled by a picker answers with the change it just had
  document.body.addEventListener('htmx:afterSettle', (e) => {
    const t = e.detail.elt || e.target;
    if (t && t.dataset && t.dataset.fire) htmx.trigger(t, 'change');
  });

  // copy a value
  document.addEventListener('click', (e) => {
    const c = e.target.closest('[data-copy]');
    if (c) { navigator.clipboard && navigator.clipboard.writeText(c.dataset.copy).then(() => toast(boot.copied || 'Copied')); }
  });
})();
