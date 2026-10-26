// Floating "Download this video" button, shown when the background worker
// detects a video/audio file or HLS stream on the page.
(() => {
  if (window.top !== window) return;

  let items = [];
  let panel = { enabled: true, skipSites: [] };
  const hostMatches = (host, p) => {
    p = String(p).replace(/^https?:\/\//i, "").split("/")[0].toLowerCase();
    if (!p) return false;
    if (p.includes("*")) return new RegExp("^" + p.replace(/[.]/g, "\\.").replace(/\*/g, ".*") + "$").test(host);
    return host === p || host.endsWith("." + p);
  };
  const panelAllowed = () =>
    panel.enabled !== false && !(panel.skipSites || []).some((s) => hostMatches(location.hostname.toLowerCase(), s));

  const fmtSize = (n) => {
    if (!n) return "";
    const u = ["B", "KB", "MB", "GB"];
    let i = 0;
    while (n >= 1024 && i < u.length - 1) {
      n /= 1024;
      i++;
    }
    return `${n.toFixed(i ? 1 : 0)} ${u[i]}`;
  };

  const cleanTitle = () =>
    (document.title || "video").replace(/[<>:"/\\|?*\u0000-\u001f]/g, "_").trim().slice(0, 120) || "video";

  function biggestVideo() {
    let best = null;
    let score = 0;
    for (const v of document.querySelectorAll("video")) {
      const r = v.getBoundingClientRect();
      if (r.width < 200 || r.height < 100) continue;
      const visible = Math.max(0, Math.min(r.bottom, innerHeight) - Math.max(r.top, 0));
      if (visible < 60) continue;
      const s = r.width * visible * (!v.paused && !v.ended ? 4 : 1);
      if (s > score) {
        best = v;
        score = s;
      }
    }
    return best;
  }

  const VIDEO_PAGE = [
    /youtube\.com\/(watch|shorts\/|live\/)/i,
    /tiktok\.com\/@[^/]+\/(video|photo)\//i,
    /instagram\.com\/(reels?|p|tv)\//i,
    /facebook\.com\/(.+\/videos\/|watch|reel\/|share\/[vr]\/)/i,
    /fb\.watch\//i,
    /(twitter|x)\.com\/[^/]+\/status\//i,
    /vimeo\.com\/\d+/i,
    /dailymotion\.com\/video\//i,
    /reddit\.com\/r\/[^/]+\/comments\//i,
    /twitch\.tv\/(videos\/|[^/]+\/clip\/)/i,
    /pinterest\.[a-z.]+\/pin\//i,
    /snapchat\.com\/(spotlight|add)\//i,
    /linkedin\.com\/(posts|feed\/update)\//i,
  ];
  const isVideoPage = (u) => VIDEO_PAGE.some((r) => r.test(u));

  function linkFor(video) {
    if (isVideoPage(location.href)) return location.href;
    return ownLink(video) || location.href;
  }

  function ownLink(video) {
    let el = video;
    for (let depth = 0; el && depth < 25; depth++, el = el.parentElement) {
      const links = el.querySelectorAll ? el.querySelectorAll("a[href]") : [];
      for (const a of links) {
        if (isVideoPage(a.href)) return a.href.split("#")[0];
      }
      if (el.tagName === "ARTICLE") break;
    }
    return "";
  }

  let lastPage = "";
  function watchVideos() {
    const v = biggestVideo();
    if (!v) return;
    if (v.paused && !(v.duration > 3) && !v.currentSrc) return;
    const page = linkFor(v);
    if (page !== lastPage) {
      lastPage = page;
      chrome.runtime.sendMessage({ type: "dios-video-seen", page }).catch?.(() => {});
    }
  }

  // ---------------------------------------------------------------- bubbles
  const bubbles = new Map();
  const ZOOM = { small: 0.85, normal: 1, large: 1.25 };
  let zoom = 1;
  const applyZoom = () => {
    for (const b of bubbles.values()) b.host.style.setProperty("--z", String(zoom));
  };
  try {
    chrome.storage.local.get({ bubbleSize: "normal" }, (o) => {
      zoom = ZOOM[o && o.bubbleSize] || 1;
      applyZoom();
    });
    chrome.storage.onChanged.addListener((ch) => {
      if (ch.bubbleSize) {
        zoom = ZOOM[ch.bubbleSize.newValue] || 1;
        applyZoom();
        for (const b of bubbles.values()) placeOne(b);
      }
    });
  } catch (_) {}

  const IDLE_MS = 2500;

  const STYLE = `
    :host {
      all: initial;
      color-scheme: dark;
      font-family: Inter, "Segoe UI", system-ui, sans-serif;
    }

    * { box-sizing: border-box; }

    .wrap {
      display: inline-block;
      color: #f1f1f1;
      font: 13px/1.4 Inter, "Segoe UI", system-ui, sans-serif;
      zoom: var(--z, 1);
      -webkit-font-smoothing: antialiased;
    }

    .btn {
      position: relative;
      display: inline-flex;
      align-items: center;
      min-height: 40px;
      color: #fff;
      background: rgba(28, 28, 30, 0.96);
      border: 1px solid rgba(255, 255, 255, 0.14);
      backdrop-filter: blur(18px);
      -webkit-backdrop-filter: blur(18px);
      cursor: grab;
      user-select: none;
      white-space: nowrap;
      font: inherit;
      font-weight: 650;
      letter-spacing: -0.15px;
      overflow: hidden;
      border-radius: 12px;
      padding: 0 13px;
      gap: 9px;
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.28), 0 12px 32px rgba(0, 0, 0, 0.18), inset 0 1px rgba(255, 255, 255, 0.06);
      will-change: padding, opacity, transform, box-shadow, border-radius, gap;
      transition:
        background 160ms ease, border-color 160ms ease, box-shadow 160ms ease,
        transform 160ms ease, opacity 0.4s cubic-bezier(0.2, 0.8, 0.2, 1),
        padding 0.4s cubic-bezier(0.2, 0.8, 0.2, 1),
        border-radius 0.4s cubic-bezier(0.2, 0.8, 0.2, 1),
        gap 0.4s cubic-bezier(0.2, 0.8, 0.2, 1);
    }

    .btn:hover {
      background: #303033;
      border-color: rgba(255, 255, 255, 0.25);
      box-shadow: 0 6px 22px rgba(0, 0, 0, 0.35);
      transform: translateY(-1px);
    }
    .btn:active { transform: scale(0.97); }
    .btn:focus-visible, .item:focus-visible, .x:focus-visible {
      outline: 2px solid #3ea6ff; outline-offset: 3px;
    }
    .btn.drag {
      cursor: grabbing;
      transform: scale(1.025);
      box-shadow: 0 10px 30px rgba(0, 0, 0, 0.4);
    }
    .btn.idle {
      opacity: 0.15;
      padding: 0;
      gap: 0;
      border-radius: 50%;
      box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
    }
    .ico-wrap {
      display: flex; align-items: center; justify-content: center;
      width: 40px; height: 40px; flex-shrink: 0;
    }
    .ico {
      width: 28px; height: 28px; flex: none;
      filter: drop-shadow(0 1px 2px rgba(0, 0, 0, 0.15));
    }
    .content {
      display: inline-flex; align-items: center; gap: 6px;
      overflow: hidden; white-space: nowrap; max-width: 300px;
      opacity: 1; transform: translateX(0);
      transition: max-width 0.4s cubic-bezier(0.2, 0.8, 0.2, 1),
                  opacity 0.3s cubic-bezier(0.2, 0.8, 0.2, 1),
                  transform 0.3s cubic-bezier(0.2, 0.8, 0.2, 1);
    }
    .btn.idle .content { max-width: 0; opacity: 0; transform: translateX(-10px); }
    .x-wrap {
      display: inline-flex; align-items: center; justify-content: center;
      overflow: hidden; max-width: 32px; opacity: 1; transform: translateX(0);
      transition: max-width 0.4s cubic-bezier(0.2, 0.8, 0.2, 1),
                  opacity 0.3s cubic-bezier(0.2, 0.8, 0.2, 1),
                  transform 0.3s cubic-bezier(0.2, 0.8, 0.2, 1);
    }
    .btn.idle .x-wrap { max-width: 0; opacity: 0; transform: translateX(10px); }
    .x {
      display: inline-flex; align-items: center; justify-content: center;
      width: 24px; height: 24px; color: #aaa; border-radius: 50%;
      cursor: pointer; font-size: 15px; flex-shrink: 0;
    }
    .x:hover { color: #fff; background: rgba(255, 255, 255, 0.14); }

    .list {
      width: 320px;
      max-width: min(320px, calc(100vw - 16px));
      max-height: min(420px, calc(100vh - 64px));
      margin-top: 9px;
      overflow-x: hidden; overflow-y: auto;
      overscroll-behavior: contain;
      scrollbar-width: thin; scrollbar-color: #555 transparent;
      color: #f1f1f1;
      background: #212121;
      border: 1px solid rgba(255, 255, 255, 0.10);
      border-radius: 12px;
      box-shadow: 0 12px 40px rgba(0, 0, 0, 0.42), 0 2px 8px rgba(0, 0, 0, 0.2);
      visibility: hidden; opacity: 0;
      transform: translateY(-8px) scale(0.96);
      pointer-events: none;
      transition: opacity 0.25s cubic-bezier(0.2, 0.8, 0.2, 1),
                  transform 0.25s cubic-bezier(0.2, 0.8, 0.2, 1),
                  visibility 0s linear 0.25s;
    }
    .list.open {
      visibility: visible; opacity: 1;
      transform: translateY(0) scale(1);
      pointer-events: auto;
      transition: opacity 0.25s cubic-bezier(0.2, 0.8, 0.2, 1),
                  transform 0.25s cubic-bezier(0.2, 0.8, 0.2, 1),
                  visibility 0s linear 0s;
    }

    .item {
      position: relative;
      display: flex; align-items: flex-start; gap: 10px;
      padding: 11px 14px;
      border-bottom: 1px solid rgba(255, 255, 255, 0.075);
      cursor: pointer;
      transition: background 120ms ease;
    }
    .item:last-child { border-bottom: 0; }
    .item:hover { background: #383838; }
    .item:active { background: #454545; }

    /* Format badge: MP4 / MP3 / WEBM / M4A etc. */
    .fmt {
      flex-shrink: 0;
      min-width: 38px;
      padding: 2px 6px;
      margin-top: 1px;
      text-align: center;
      font-size: 10px;
      font-weight: 700;
      letter-spacing: 0.3px;
      text-transform: uppercase;
      color: #c8e6ff;
      background: rgba(62, 166, 255, 0.16);
      border: 1px solid rgba(62, 166, 255, 0.30);
      border-radius: 4px;
    }
    .item.audio .fmt {
      color: #c9f0d6;
      background: rgba(76, 217, 123, 0.16);
      border-color: rgba(76, 217, 123, 0.32);
    }

    .txt { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 3px; }
    .t {
      color: #f1f1f1; font-size: 13px; font-weight: 650; line-height: 1.4;
      overflow-wrap: anywhere;
    }
    .s {
      color: #aaa; font-size: 11px; line-height: 1.5;
      overflow-wrap: anywhere;
    }

    .off { cursor: not-allowed; opacity: 0.45; }
    .off:hover { background: transparent; }

    .msg {
      display: flex; align-items: flex-start; gap: 8px;
      padding: 14px 15px; color: #a8e6bc;
      font-size: 12px; line-height: 1.6; overflow-wrap: anywhere;
    }
    .err { color: #ffaaa5; }

    @media (max-width: 420px) {
      .list { width: min(290px, calc(100vw - 16px)); }
      .btn { min-height: 38px; }
    }
    @media (prefers-reduced-motion: reduce) {
      *, *::before, *::after {
        animation-duration: 0.01ms !important;
        transition-duration: 0.01ms !important;
      }
    }
  `;

  function isMain(video) { return video === biggestVideo(); }

  function directSrc(video) {
    const src = video.currentSrc || video.src || "";
    return /^https?:/i.test(src) && !/\.m3u8|\.mpd/i.test(src) ? src : "";
  }

  const POS_KEY = "bubblePos:" + location.hostname;
  let savedPositions = {};
  try {
    chrome.storage.local.get({ [POS_KEY]: {} }, (o) => {
      savedPositions = (o && o[POS_KEY]) || {};
      for (const b of bubbles.values()) placeOne(b);
    });
  } catch (_) {}

  function rememberPos(b, key) {
    if (b.dx == null || b.dy == null) return;
    savedPositions[key] = { x: b.dx, y: b.dy };
    try { chrome.storage.local.set({ [POS_KEY]: savedPositions }); } catch (_) {}
  }

  function restorePos(b) {
    if (!lastPage) return;
    const p = savedPositions[lastPage];
    if (p && typeof p.x === "number" && typeof p.y === "number") {
      b.dx = p.x;
      b.dy = p.y;
    }
  }

  function makeBubble(video) {
    const host = document.createElement("div");
    host.style.all = "initial";
    host.style.position = "fixed";
    host.style.zIndex = "2147483647";
    host.style.setProperty("--z", String(zoom));
    host.style.willChange = "left, top";
    const root = host.attachShadow({ mode: "closed" });

    const el = (tag, props = {}, ...kids) => {
      const e = document.createElement(tag);
      for (const [k, v] of Object.entries(props)) {
        if (k === "text") e.textContent = v;
        else e.setAttribute(k, v);
      }
      e.append(...kids);
      return e;
    };

    // ---- Download icon: arrow into tray (no background) ----
    const svgNs = "http://www.w3.org/2000/svg";
    const svg = document.createElementNS(svgNs, "svg");
    svg.setAttribute("class", "ico");
    svg.setAttribute("viewBox", "0 0 24 24");
    svg.setAttribute("fill", "none");
    svg.setAttribute("aria-hidden", "true");
    // Arrow shaft
    const shaft = document.createElementNS(svgNs, "path");
    shaft.setAttribute("d", "M12 3v11");
    shaft.setAttribute("stroke", "#fff");
    shaft.setAttribute("stroke-width", "2.2");
    shaft.setAttribute("stroke-linecap", "round");
    // Arrow head
    const head = document.createElementNS(svgNs, "path");
    head.setAttribute("d", "M7.5 9.5 12 14l4.5-4.5");
    head.setAttribute("stroke", "#fff");
    head.setAttribute("stroke-width", "2.2");
    head.setAttribute("stroke-linecap", "round");
    head.setAttribute("stroke-linejoin", "round");
    // Tray
    const tray = document.createElementNS(svgNs, "path");
    tray.setAttribute("d", "M5.5 17v1.4a2.1 2.1 0 0 0 2.1 2.1h8.8a2.1 2.1 0 0 0 2.1-2.1V17");
    tray.setAttribute("stroke", "#fff");
    tray.setAttribute("stroke-width", "2.2");
    tray.setAttribute("stroke-linecap", "round");
    svg.append(shaft, head, tray);

    const list = el("div", { class: "list", id: "list", role: "listbox", "aria-label": "Available formats", hidden: true });

    const btn = el(
      "button",
      {
        class: "btn",
        id: "main",
        type: "button",
        "aria-haspopup": "listbox",
        "aria-expanded": "false",
        "aria-label": "Download this video",
      },
      el("div", { class: "ico-wrap" }, svg),
      el("span", { class: "content" },
        el("span", { id: "label", class: "label", text: "Download this video" }),
        // el("span", { class: "caret", text: "▼", "aria-hidden": "true" })
      ),
      el("span", { class: "x-wrap" },
        el("span", {
          class: "x", id: "close", role: "button", tabindex: "0",
          title: "Hide", "aria-label": "Hide download button", text: "✕",
        })
      )
    );

    root.append(el("style", { text: STYLE }), el("div", { class: "wrap" }, btn, list));
    const b = {
      host, root, video, open: false, own: null, loading: false,
      dx: null, dy: null, idle: false, idleTimer: null, closeTimer: null
    };
    const mainBtn = root.getElementById("main");
    restorePos(b);

    function setIdle(on) {
      if (b.open) return;
      b.idle = on;
      mainBtn.classList.toggle("idle", on);
    }
    function resetIdle() {
      if (b.idleTimer) clearTimeout(b.idleTimer);
      setIdle(false);
      b.idleTimer = setTimeout(() => setIdle(true), IDLE_MS);
    }
    b.resetIdle = resetIdle;
    b.setIdle = setIdle;

    let down = null;
    mainBtn.addEventListener("pointerdown", (e) => {
      if (e.button !== 0 || e.target.id === "close") return;
      mainBtn.setPointerCapture?.(e.pointerId);
      const hr = host.getBoundingClientRect();
      down = { x: e.clientX, y: e.clientY, ox: e.clientX - hr.left, oy: e.clientY - hr.top, moved: false, id: e.pointerId };
      resetIdle();
      e.preventDefault();
    });
    mainBtn.addEventListener("pointermove", (e) => {
      if (!down || e.pointerId !== down.id) return;
      if (!down.moved && Math.abs(e.clientX - down.x) + Math.abs(e.clientY - down.y) < 5) return;
      down.moved = true;
      mainBtn.classList.add("drag");
      const r = video.getBoundingClientRect();
      b.dx = e.clientX - down.ox - r.left;
      b.dy = e.clientY - down.oy - r.top;
      placeOne(b);
    });
    mainBtn.addEventListener("pointerup", (e) => {
      if (!down || e.pointerId !== down.id) return;
      const wasDrag = down.moved;
      down = null;
      mainBtn.classList.remove("drag");
      mainBtn.releasePointerCapture?.(e.pointerId);
      if (wasDrag) rememberPos(b, lastPage);
      else clicked(b);
      resetIdle();
    });
    mainBtn.addEventListener("pointercancel", () => {
      if (!down) return;
      down = null;
      mainBtn.classList.remove("drag");
      resetIdle();
    });
    mainBtn.addEventListener("pointerenter", resetIdle);
    mainBtn.addEventListener("focus", resetIdle);
    mainBtn.addEventListener("keydown", (e) => {
      if (e.key === "Escape") closeOne(b);
      resetIdle();
    });

    root.getElementById("close").addEventListener("click", (e) => {
      e.stopPropagation();
      b.dismissed = true;
      host.remove();
    });
    root.getElementById("close").addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault(); e.stopPropagation();
        b.dismissed = true;
        host.remove();
      }
    });

    document.documentElement.appendChild(host);
    bubbles.set(video, b);
    renderOne(b);
    placeOne(b);
    resetIdle();
    return b;
  }

  function itemsOf(b) {
    if (b.own) return b.own;
    return isMain(b.video) ? items : [];
  }

  // ---- Format helpers ----
  function isAudio(m) {
    if (m.audioOnly) return true;
    const ext = String(m.ext || m.format || "").toLowerCase();
    return /^(mp3|m4a|aac|ogg|opus|flac|wav|weba)$/.test(ext);
  }

  function fmtLabel(m) {
    const ext = String(m.ext || m.format || "").toLowerCase();
    if (ext) return ext.toUpperCase();
    return m.audioOnly ? "AUDIO" : "VIDEO";
  }

  function subFor(m) {
    const bits = [];
    const quality = m.quality || m.resolution || m.height || "";
    if (quality) bits.push(String(quality));
    const format = m.format || m.ext || "";
    if (format) bits.push(String(format).toUpperCase());
    // bitrate for audio
    const br = m.bitrate || m.kbps || m.audioBitrate || 0;
    if (br) bits.push(typeof br === "number" ? `${br} kbps` : String(br));
    const size = m.size || m.bytes || m.filesize || m.contentLength || 0;
    if (size) bits.push(fmtSize(size));
    if (m.audioOnly) bits.push("audio only");
    if (m.videoOnly) bits.push("video only");
    if (bits.length) return bits.join(" · ");
    return m.sub || "";
  }

  // Prefer best video; only fall back to best audio if no video exists.
  function recommendedIndex(list) {
    const usable = list
      .map((m, i) => ({ m, i }))
      .filter(({ m }) => !m.disabled && m.yt !== true && !isAudio(m));
    if (!usable.length) return -1;
    let best = -1;
    let bestScore = -1;
    for (const { m, i } of usable) {
      const h = m.height || parseInt(String(m.quality || m.resolution || "").replace(/\D+/g, ""), 10) || 0;
      const s = m.size || m.bytes || m.filesize || m.contentLength || 0;
      const score = h * 1e6 + s;
      if (score > bestScore) {
        bestScore = score;
        best = i;
      }
    }
    return best;
  }

  function clicked(b) {
    const list = itemsOf(b);
    if (list.length === 1 && !list[0].disabled && !list[0].yt) {
      download(b, list[0]);
      return;
    }
    if (!list.length && !b.loading) {
      b.loading = true;
      b.open = true;
      renderOne(b, "Looking for the video…");
      chrome.runtime.sendMessage(
        {
          type: "dios-resolve-video",
          page: ownLink(b.video) || (directSrc(b.video) ? "" : location.href),
          src: directSrc(b.video),
        },
        (res) => {
          b.loading = false;
          if (chrome.runtime.lastError || !res) return renderOne(b, "DIOS Downloader is not running — open the app and try again.", true);
          if (res.error) return renderOne(b, res.error, true);
          b.own = res.items || [];
          if (b.own.length === 1 && !b.own[0].disabled && !b.own[0].yt) return download(b, b.own[0]);
          renderOne(b);
        }
      );
      return;
    }
    b.open = !b.open;
    if (b.open) b.setIdle(false);
    renderOne(b);
  }

  function renderOne(b, note, isError) {
    const list = itemsOf(b);
    const usable = list.filter((i) => !i.disabled).length;
    const mainBtn = b.root.getElementById("main");
    b.root.getElementById("label").textContent = usable > 1 ? `Download this video (${usable})` : "Download this video";
    mainBtn.setAttribute("aria-expanded", b.open ? "true" : "false");

    const box = b.root.getElementById("list");
    if (b.open) {
      if (b.closeTimer) clearTimeout(b.closeTimer);
      box.removeAttribute("hidden");
      void box.offsetWidth;
      box.classList.add("open");
    } else {
      box.classList.remove("open");
      b.closeTimer = setTimeout(() => {
        if (!b.open) box.setAttribute("hidden", "");
      }, 260);
    }

    if (b.open) b.setIdle(false);
    else if (b.resetIdle) b.resetIdle();

    if (note) {
      box.replaceChildren();
      const d = document.createElement("div");
      d.className = "msg" + (isError ? " err" : "");
      d.textContent = note;
      box.appendChild(d);
      return;
    }

    box.replaceChildren();
    const recommended = recommendedIndex(list);
    list.forEach((m, i) => {
      const div = document.createElement("div");
      const audio = isAudio(m);
      div.className = "item" + (m.disabled ? " off" : "") + (audio ? " audio" : "");
      div.setAttribute("role", "option");
      if (m.disabled) div.setAttribute("aria-disabled", "true");
      else div.tabIndex = 0;

      // Format badge (MP4 / MP3 / WEBM / M4A ...)
      const badge = document.createElement("span");
      badge.className = "fmt";
      badge.textContent = fmtLabel(m);

      const txt = document.createElement("div");
      txt.className = "txt";

      const t = document.createElement("div");
      t.className = "t";
      t.textContent = m.label || (audio ? "Audio track" : "Video");
      if (i === recommended) t.textContent += " ★";

      const s = document.createElement("div");
      s.className = "s";
      const sub = subFor(m);
      s.textContent = sub.length > 90 ? sub.slice(0, 90) + "…" : sub;

      txt.append(t, s);
      div.append(badge, txt);
      if (!m.disabled) {
        div.addEventListener("click", () => download(b, m));
        div.addEventListener("keydown", (e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            download(b, m);
          }
        });
      }
      box.appendChild(div);
    });
  }

  function placeOne(b) {
    const r = b.video.getBoundingClientRect();
    const onScreen = r.bottom > 0 && r.top < innerHeight && r.width >= 200 && r.height >= 100 && b.video.isConnected;
    b.host.style.display = onScreen ? "block" : "none";
    if (!onScreen) return;

    const w = b.host.getBoundingClientRect().width || 170;
    // Default to LEFT side of the video; a saved/dragged position still wins.
    let x = b.dx == null ? r.left + 8 : r.left + b.dx;
    let y = b.dy == null ? r.top + 8 : r.top + b.dy;

    x = Math.max(r.left, Math.min(x, r.right - 60));
    y = Math.max(r.top, Math.min(y, r.bottom - 24));

    b.host.style.left = `${Math.max(4, Math.min(x, innerWidth - w))}px`;
    b.host.style.top = `${Math.max(4, Math.min(y, innerHeight - 28))}px`;
  }

  function place() {
    if (!panelAllowed()) return;
    const vids = [...document.querySelectorAll("video")].filter((v) => {
      const r = v.getBoundingClientRect();
      return r.width >= 200 && r.height >= 100;
    });

    for (const [v, b] of bubbles) {
      if (!v.isConnected) {
        if (b.idleTimer) clearTimeout(b.idleTimer);
        if (b.closeTimer) clearTimeout(b.closeTimer);
        b.host.remove();
        bubbles.delete(v);
      }
    }

    for (const v of vids.slice(0, 12)) {
      let b = bubbles.get(v);
      if (!b) {
        if (isMain(v) ? !items.length : v.paused && !(v.duration > 3) && !directSrc(v)) continue;
        b = makeBubble(v);
      }
      if (!b.dismissed) placeOne(b);
    }
  }

  function closeOne(b) {
    b.open = false;
    renderOne(b);
  }

  let flashTimer = null;
  function flashOne(b, text, isError) {
    if (flashTimer) clearTimeout(flashTimer);
    b.open = true;
    b.setIdle(false);
    renderOne(b, text, isError);
    flashTimer = setTimeout(() => {
      b.open = false;
      renderOne(b);
    }, 3200);
  }

  function download(b, m) {
    const audio = isAudio(m);
    const ext = m.ext || m.format || (audio ? "mp3" : "mp4");
    const fileName = `${cleanTitle()}.${ext}`;
    chrome.runtime.sendMessage({ type: "dios-download", item: m, title: document.title, fileName }, (res) => {
      if (chrome.runtime.lastError || !res || !res.ok) {
        flashOne(b, "DIOS Downloader is not running — open the app and try again.", true);
      } else {
        flashOne(b, "Sent to DIOS Downloader ✔", false);
      }
    });
  }

  function update(newItems) {
    items = newItems || [];
    if (!items.length) {
      for (const b of bubbles.values()) {
        if (b.idleTimer) clearTimeout(b.idleTimer);
        if (b.closeTimer) clearTimeout(b.closeTimer);
        b.host.remove();
      }
      bubbles.clear();
      return;
    }
    if (!panelAllowed()) return;
    place();
    for (const b of bubbles.values()) if (!b.own && !b.loading) renderOne(b);
  }

  function collectLinks(selectionOnly) {
    const sel = selectionOnly ? getSelection() : null;
    const inSel = (el) => !sel || (sel.rangeCount > 0 && sel.containsNode(el, true));
    const out = [];
    const seen = new Set();
    const add = (url, text) => {
      if (!url || !/^https?:/i.test(url)) return;
      url = url.split("#")[0];
      if (seen.has(url)) return;
      seen.add(url);
      out.push({ url, text: (text || "").replace(/\s+/g, " ").trim().slice(0, 200) });
    };
    for (const a of document.querySelectorAll("a[href]")) if (inSel(a)) add(a.href, a.innerText || a.title);
    for (const el of document.querySelectorAll("img[src], video[src], audio[src], source[src], embed[src]")) {
      if (inSel(el)) add(el.src, el.alt || el.title || "");
    }
    return { page: location.href, links: out };
  }

  chrome.runtime.onMessage.addListener((msg, _sender, sendResponse) => {
    if (msg.type === "dios-media") update(msg.items);
    if (msg.type === "dios-collect-links") {
      sendResponse(collectLinks(!!msg.selection));
      return true;
    }
    if (msg.type === "dios-open-panel") {
      const v = biggestVideo();
      let b = v && bubbles.get(v);
      if (v && !b) b = makeBubble(v);
      if (b) {
        b.dismissed = false;
        document.documentElement.appendChild(b.host);
        b.open = true;
        b.setIdle(false);
        renderOne(b);
        placeOne(b);
      }
    }
    return false;
  });

  chrome.runtime.sendMessage({ type: "dios-config" }, (res) => {
    if (!chrome.runtime.lastError && res) {
      panel = res;
      if (!panelAllowed()) {
        for (const b of bubbles.values()) {
          if (b.idleTimer) clearTimeout(b.idleTimer);
          if (b.closeTimer) clearTimeout(b.closeTimer);
          b.host.remove();
        }
        bubbles.clear();
      }
    }
  });

  addEventListener("mousedown", (e) => {
    chrome.runtime.sendMessage({ type: "dios-keys", alt: e.altKey, ctrl: e.ctrlKey, shift: e.shiftKey }).catch?.(() => {});
  }, true);

  // Close any open bubble when clicking outside of it.
  addEventListener("pointerdown", (e) => {
    for (const b of bubbles.values()) {
      if (!b.open) continue;
      // e.target retargets to the host element for clicks inside a closed shadow root,
      // so this correctly detects clicks inside the bubble.
      if (b.host === e.target || b.host.contains(e.target)) continue;
      closeOne(b);
    }
  }, true);

  chrome.runtime.sendMessage({ type: "dios-media-request" }, (res) => {
    if (!chrome.runtime.lastError) update(res);
  });

  addEventListener("keydown", (e) => {
    if (e.key !== "Escape") return;
    let handled = false;
    for (const b of bubbles.values()) {
      if (b.open) {
        closeOne(b);
        handled = true;
      }
    }
    if (handled) e.stopPropagation();
  });

  addEventListener("scroll", place, { passive: true });
  addEventListener("resize", place, { passive: true });
  setInterval(place, 700);
  setInterval(watchVideos, 1500);
  document.addEventListener("play", () => setTimeout(watchVideos, 300), true);
  addEventListener("popstate", () => setTimeout(watchVideos, 500));
})();