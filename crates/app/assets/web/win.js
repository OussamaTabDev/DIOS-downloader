/* Batiie – the script of a window of its own (not the main window).
   It closes the window when the page says so, asks the page what to do before the
   operating system closes it, and keeps the title and the look up to date. */
(() => {
  'use strict';
  const $ = (s, r = document) => r.querySelector(s);
  const $$ = (s, r = document) => Array.from(r.querySelectorAll(s));
  const boot = (() => { try { return JSON.parse($('#batiie-boot').textContent); } catch (_) { return {}; } })();
  const ipc = (m) => { try { if (window.ipc) window.ipc.postMessage(m); else if (window.opener || history.length <= 1) window.close(); } catch (_) {} };
  const ajax = (method, url, values) => htmx.ajax(method, url, { source: 'body', swap: 'none', values: values || {} });

  // ---- the window's title follows the page
  const setTitle = () => {
    const el = $('[data-title]'); const t = el ? el.dataset.title : document.title;
    if (t) { document.title = t; ipc('title:' + t); }
  };
  setTitle();

  // ---- closing
  // the operating system's close button: the page decides first (cancel a new download, stop
  // a running one), then the window goes
  const target = () => $('[data-onclose]');
  window.batiieBeforeClose = () => {
    const el = target(); const url = el && el.dataset.onclose;
    if (!url) { ipc('close'); return; }
    fetch(url, { method: 'POST', headers: { 'X-Batiie': '1' } }).catch(() => {}).finally(() => ipc('close'));
  };
  // Cancel / the "x" of the dialog: the same as the system button
  document.addEventListener('click', (e) => {
    if (e.target.closest('[data-close]')) { e.preventDefault(); window.batiieBeforeClose(); }
  });
  // the server finished with the window
  const closeNow = (id) => { const d = $('dialog.dlg'); if (!id || !d || d.id === id || document.body.dataset.label === id) ipc('close'); };
  document.body.addEventListener('dlgClose', (e) => closeNow(typeof e.detail === 'string' ? e.detail : (e.detail && e.detail.value)));
  document.body.addEventListener('htmx:sseBeforeMessage', (e) => {
    const type = String(e.detail.type || '').replace(/^batiie-/, '');
    if (type === 'close') { e.preventDefault(); closeNow(e.detail.data); }
  });
  // Escape closes a question (not a progress window)
  document.addEventListener('keydown', (e) => { if (e.key === 'Escape' && $('dialog.dlg')) { e.preventDefault(); window.batiieBeforeClose(); } });

  // ---- the look
  const applyLook = (s) => {
    if (!s) return;
    const [theme, fs, family, lang] = s.split('|');
    const h = document.documentElement;
    h.dataset.theme = theme;
    h.style.setProperty('--fs', String(Math.min(2, Math.max(0.7, (+fs || 100) / 100))));
    if (family) h.style.setProperty('--font', "'" + family.replace(/[^\w \-]/g, '') + "',system-ui,sans-serif"); else h.style.removeProperty('--font');
    if (lang && lang !== h.lang) location.reload();
  };
  document.body.addEventListener('htmx:oobAfterSwap', (e) => {
    const old = e.detail.target || e.target; const t = old && old.id ? document.getElementById(old.id) : null;
    if (t && t.id === 'look') applyLook(t.dataset.look);
  });

  // ---- helpers the dialogs use (the same as in the main window)
  const syncDeps = (root) => $$('[data-enable]', root || document).forEach((el) => {
    const src = $(el.dataset.enable); if (!src) return;
    const on = src.type === 'checkbox' ? src.checked : !!src.value;
    $$('input,select,textarea,button', el).forEach((c) => { if (c !== src) c.disabled = !on; });
    el.classList.toggle('disabled', !on);
  });
  syncDeps();
  document.addEventListener('change', () => syncDeps());
  document.body.addEventListener('htmx:afterSettle', () => syncDeps());
  document.addEventListener('click', (e) => {
    const b = e.target.closest('[data-checkall]'); if (!b) return;
    $$(b.dataset.checkall).forEach((c) => { c.checked = b.dataset.state !== '0'; c.dispatchEvent(new Event('change', { bubbles: true })); });
    const c = e.target.closest('[data-clear]');
    if (c) { const i = $(c.dataset.clear); if (i) { i.value = ''; htmx.trigger(i, 'change'); } }
  });
  document.addEventListener('click', (e) => {
    const c = e.target.closest('[data-clear]'); if (c) { const i = $(c.dataset.clear); if (i) { i.value = ''; htmx.trigger(i, 'change'); } }
    const a = e.target.closest('a[data-ext]'); if (a) { e.preventDefault(); ajax('POST', '/act/openurl', { url: a.dataset.ext }); }
    const cp = e.target.closest('[data-copy]'); if (cp && navigator.clipboard) navigator.clipboard.writeText(cp.dataset.copy).catch(() => {});
  });
  document.body.addEventListener('htmx:afterSettle', (e) => { const t = e.detail.elt || e.target; if (t && t.dataset && t.dataset.fire) htmx.trigger(t, 'change'); });
  // Enter in a text box = the main button of the form
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && e.target.matches('input.in[type=text],input.in[type=password],input.in.num')) {
      const f = e.target.closest('form'); if (f && !e.target.closest('[data-noenter]')) { e.preventDefault(); const b = f.querySelector('[data-enter]') || f.querySelector('button.primary[type=submit]'); if (b) b.click(); }
    }
  });
  // dialogs that open other dialogs (a button with hx-target #modal-host): the server opens the window
  const first = $('dialog.dlg [autofocus], dialog.dlg input.in:not([readonly]), dialog.dlg textarea.in');
  if (first) { first.focus({ preventScroll: true }); if (first.tagName === 'INPUT' && first.type === 'text' && first.dataset.noselect === undefined && first.select) first.select(); }
})();
