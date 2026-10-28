(() => {
  "use strict";
  if (window.top !== window || location.origin !== "https://web.whatsapp.com") return null;
  if (globalThis.relayWhatsApp) return globalThis.relayWhatsApp;

  const media = matchMedia("(max-width: 748px)");
  const attributes = ["path", "root", "list", "chat", "app", "header", "heading", "composer", "column", "messages", "fixed", "nav"].map(name => `data-relay-wa-${name}`);
  const rowSelector = '[role="row"], [role="listitem"], [role="option"], [data-testid="cell-frame-container"]';
  let enabled = false;
  let initialized = false;
  let wantChat = false;
  let pendingSelection = false;
  let frame = 0;
  let observedRoot = null;
  let marked = new Set();
  let layout = null;
  let back = null;
  let style = null;
  let lastRow = null;
  let navigation = false;
  let navButton = null;
  let scrim = null;
  let focusList = false;
  let focusChat = false;

  const css = `
@media (max-width: 748px) {
  html[data-relay-wa-pane], html[data-relay-wa-pane] body {
    min-width: 0 !important; width: 100% !important; max-width: 100% !important;
    overflow-x: hidden !important;
  }
  html[data-relay-wa-pane] [data-relay-wa-path] {
    min-width: 0 !important; width: 100% !important; max-width: 100% !important;
    min-height: 0 !important; height: 100% !important; max-height: 100% !important;
    box-sizing: border-box !important;
  }
  html[data-relay-wa-pane] [data-relay-wa-app] { position: relative !important; }
  html[data-relay-wa-pane] [data-relay-wa-root] { position: absolute !important; inset: 0 !important; }
  html[data-relay-wa-pane] #side, html[data-relay-wa-pane] #main,
  html[data-relay-wa-pane] [data-relay-wa-column] {
    display: flex !important; flex-direction: column !important; min-width: 0 !important;
    min-height: 0 !important; width: 100% !important; height: 100% !important; overflow: hidden !important;
  }
  html[data-relay-wa-pane] #side { border-inline-end: 0 !important; }
  html[data-relay-wa-pane] [data-relay-wa-header] {
    display: flex !important; align-items: center !important; flex-wrap: nowrap !important;
    flex: 0 0 56px !important; min-width: 0 !important; width: 100% !important;
    min-height: 56px !important; height: 56px !important; max-height: 56px !important;
    padding: 4px 8px !important; gap: 4px !important; box-sizing: border-box !important;
  }
  html[data-relay-wa-pane] [data-relay-wa-header] div { min-width: 0 !important; }
  html[data-relay-wa-pane] [data-relay-wa-heading] {
    flex: 1 1 0% !important; min-width: 0 !important; max-width: 100% !important; overflow: hidden !important;
  }
  html[data-relay-wa-pane] [data-relay-wa-heading] span {
    overflow: hidden !important; text-overflow: ellipsis !important; white-space: nowrap !important;
  }
  html[data-relay-wa-pane] [data-relay-wa-heading] > span { display: block; }
  html[data-relay-wa-pane] [data-relay-wa-header] button,
  html[data-relay-wa-pane] [data-relay-wa-header] [role="button"],
  html[data-relay-wa-pane] [data-relay-wa-composer] button,
  html[data-relay-wa-pane] [data-relay-wa-composer] [role="button"] {
    min-width: 44px !important; min-height: 44px !important; flex-shrink: 0 !important;
    touch-action: manipulation;
  }
  html[data-relay-wa-pane] [data-relay-wa-header] [data-relay-wa-heading] {
    min-width: 0 !important; flex: 1 1 0% !important;
  }
  html[data-relay-wa-pane] #pane-side :is(button,[role="button"]) { min-width: 44px; min-height: 44px; }
  html[data-relay-wa-pane] [data-relay-wa-header] img { max-width: 40px; max-height: 40px; }
  html[data-relay-wa-pane] [data-relay-wa-header] :is(button,[role="button"]) :is(button,[role="button"]) {
    min-width: 0 !important; min-height: 0 !important;
  }
  html[data-relay-wa-pane] [data-relay-wa-fixed] { flex: 0 0 auto !important; min-height: 0 !important; }
  html[data-relay-wa-pane] [data-relay-wa-messages] {
    flex: 1 1 0% !important; min-width: 0 !important; min-height: 0 !important;
    overflow-y: auto !important; overscroll-behavior-y: contain;
  }
  html[data-relay-wa-pane] #pane-side { min-width: 0 !important; min-height: 0 !important; overscroll-behavior-y: contain; }
  html[data-relay-wa-pane] [data-relay-wa-composer] {
    flex: 0 0 auto !important; min-width: 0 !important; width: 100% !important;
    padding: 8px !important; gap: 4px !important; box-sizing: border-box !important;
  }
  html[data-relay-wa-pane] [data-relay-wa-composer] div {
    min-width: 0 !important; max-width: 100% !important;
  }
  html[data-relay-wa-pane] [data-relay-wa-composer] :is(input,textarea,[role="textbox"]) {
    min-width: 0 !important; max-width: 100% !important; font-size: max(16px, 1em);
  }
  html[data-relay-wa-pane] [data-relay-wa-composer] [contenteditable="true"] {
    max-height: min(30vh, 160px) !important; overflow-y: auto !important; overflow-wrap: anywhere;
  }
  html[data-relay-wa-pane] [data-relay-wa-nav] { display: none !important; }
  html[data-relay-wa-navigation] [data-relay-wa-nav] {
    display: flex !important; position: absolute !important; inset-block: 0 !important;
    inset-inline-start: 0 !important; width: 72px !important; min-width: 72px !important;
    flex-direction: column !important; align-items: center !important; z-index: 1001 !important;
    padding-block: 12px !important; overflow-y: auto !important;
  }
  html[data-relay-wa-navigation] [data-relay-wa-nav] :is(button,[role="button"]) { min-width: 44px !important; min-height: 44px !important; }
  #relay-wa-nav-scrim { position: absolute; inset: 0; border: 0; padding: 0; background: #0008; z-index: 1000; }
  #relay-wa-navigation, #relay-wa-back {
    -webkit-tap-highlight-color: transparent; touch-action: manipulation;
  }
  #relay-wa-navigation { appearance: none; display: inline-grid; place-items: center;
    width: 44px; height: 44px; padding: 10px; flex: 0 0 44px; margin: 0; border: 0;
    border-radius: 50%; background: transparent; color: inherit; cursor: pointer; }
  #relay-wa-navigation svg { width: 24px; height: 24px; pointer-events: none; }
  html[data-relay-wa-pane] [data-relay-wa-list],
  html[data-relay-wa-pane] [data-relay-wa-chat] {
    position: absolute !important; inset: 0 !important; margin: 0 !important;
    flex: 1 1 100% !important; z-index: 2 !important;
  }
  html[data-relay-wa-pane="list"] [data-relay-wa-chat],
  html[data-relay-wa-pane="chat"] [data-relay-wa-list] { display: none !important; }
  #relay-wa-back {
    appearance: none; display: inline-grid; place-items: center; flex: 0 0 44px;
    width: 44px; height: 44px; padding: 10px; margin: 0; order: -1;
    border: 0; border-radius: 50%; background: transparent; color: inherit; cursor: pointer;
  }
  #relay-wa-back:hover, #relay-wa-navigation:hover { background: #80808020; }
  #relay-wa-back:focus-visible, #relay-wa-navigation:focus-visible { outline: 2px solid currentColor; outline-offset: -3px; }
  #relay-wa-back svg { width: 24px; height: 24px; pointer-events: none; }
  [dir="rtl"] #relay-wa-back svg { transform: rotate(180deg); }
}`;

  function removeBack() {
    back?.remove();
    back = null;
  }

  function clearLayout() {
    document.documentElement.removeAttribute("data-relay-wa-pane");
    document.documentElement.removeAttribute("data-relay-wa-navigation");
    navButton?.remove(); navButton = null;
    scrim?.remove(); scrim = null;
    navigation = false;
    for (const node of marked) for (const name of attributes) node.removeAttribute(name);
    marked.clear();
    layout = null;
    removeBack();
  }

  function pathTo(node, ancestor) {
    const path = [];
    for (let current = node; current && path.length < 32; current = current.parentElement) {
      path.push(current);
      if (current === ancestor) return path;
    }
    return null;
  }

  function discover() {
    const app = document.getElementById("app");
    const pane = document.getElementById("pane-side");
    const side = document.getElementById("side") || pane;
    const main = document.getElementById("main");
    const header = main?.querySelector("header") || null;
    if (!app || !pane || !side || !app.contains(side) || !side.contains(pane)) return null;
    if (main && (!app.contains(main) || side.contains(main) || main.contains(side))) return null;
    if (main && !header) return null;

    let root = side.parentElement;
    if (main) {
      const mainPath = pathTo(main, app);
      const sidePath = pathTo(side, app);
      if (!mainPath || !sidePath) return null;
      const ancestors = new Set(mainPath);
      root = sidePath.find(node => ancestors.has(node));
    }
    if (!root || !app.contains(root) || root === side || root === main) return null;
    const sidePath = pathTo(side, root);
    const mainPath = main ? pathTo(main, root) : [];
    const appPath = pathTo(root, app);
    if (!sidePath || !mainPath || !appPath) return null;
    const listBranch = sidePath[sidePath.length - 2];
    const chatBranch = main ? mainPath[mainPath.length - 2] : null;
    if (!listBranch || (main && (!chatBranch || chatBranch === listBranch))) return null;
    const sideHeader = side.querySelector("header");
    const footer = main?.querySelector("footer") || null;
    const nav = [...app.querySelectorAll('nav, [role="navigation"]')].find(node =>
      !side.contains(node) && !main?.contains(node)) || null;
    const headingCandidates = header && [...header.children].filter(node =>
      node.id !== "relay-wa-back" && !node.matches('button, img, svg') && !node.querySelector('button'));
    const title = headingCandidates?.find(node => node.matches('h1,h2,[dir="auto"]') ||
      node.querySelector('span[dir="auto"],span[title],h1,h2')) ||
      headingCandidates?.find(node => node.matches('div,h1,h2') && !node.querySelector('img,svg'));
    const messageBranch = footer?.previousElementSibling;
    return { app, pane, side, main, root, listBranch, chatBranch,
      paths: [...new Set([...sidePath, ...mainPath, ...appPath])],
      header, sideHeader, footer, nav, title,
      messages: messageBranch && messageBranch !== header && !messageBranch.contains(header) ? messageBranch : null };
  }

  function mark(current) {
    const desired = new Map();
    function flag(node, name) {
      if (!node) return;
      if (!desired.has(node)) desired.set(node, new Set());
      desired.get(node).add(`data-relay-wa-${name}`);
    }
    for (const node of current.paths) flag(node, "path");
    flag(current.root, "root"); flag(current.app, "app");
    flag(current.listBranch, "list"); flag(current.chatBranch, "chat");
    flag(current.header, "header"); flag(current.sideHeader, "header");
    flag(current.title, "heading"); flag(current.footer, "composer");
    flag(current.header, "fixed"); flag(current.footer, "fixed");
    flag(current.messages, "messages"); flag(current.nav, "nav");
    for (const node of marked) {
      if (!desired.has(node)) for (const name of attributes) node.removeAttribute(name);
    }
    for (const [node, flags] of desired) {
      for (const name of attributes) {
        if (node.hasAttribute(name) !== flags.has(name)) node.toggleAttribute(name, flags.has(name));
      }
    }
    marked = new Set(desired.keys());
  }

  function svgButton(id, label, path) {
    const button = document.createElement("button");
    button.id = id; button.type = "button"; button.title = label;
    button.setAttribute("aria-label", label);
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("viewBox", "0 0 24 24"); svg.setAttribute("aria-hidden", "true");
    const arrow = document.createElementNS("http://www.w3.org/2000/svg", "path");
    for (const [name, value] of Object.entries({d:path,fill:"none",stroke:"currentColor","stroke-width":"2","stroke-linecap":"round","stroke-linejoin":"round"})) arrow.setAttribute(name,value);
    svg.append(arrow); button.append(svg);
    return button;
  }

  function syncNavigation(current) {
    if (!current.nav || !current.sideHeader || wantChat) {
      navButton?.remove(); navButton = null; navigation = false;
    } else if (navButton?.parentElement !== current.sideHeader) {
      navButton?.remove();
      navButton = svgButton("relay-wa-navigation", "WhatsApp navigation", "M4 6h16M4 12h16M4 18h16");
      navButton.addEventListener("click", event => {
        event.preventDefault(); event.stopPropagation();
        navigation = !navigation; sync();
        if (navigation) layout?.nav?.querySelector('button,[role="button"]')?.focus({preventScroll:true});
      });
      current.sideHeader.prepend(navButton);
    }
    if (document.documentElement.hasAttribute("data-relay-wa-navigation") !== navigation) {
      document.documentElement.toggleAttribute("data-relay-wa-navigation", navigation);
    }
    if (navigation && scrim?.parentElement !== current.app) {
      scrim?.remove();
      scrim = document.createElement("button"); scrim.id="relay-wa-nav-scrim";
      scrim.type="button"; scrim.setAttribute("aria-label", "Close navigation");
      scrim.addEventListener("click", () => {navigation=false;sync();navButton?.focus({preventScroll:true});});
      current.app.append(scrim);
    } else if (!navigation && scrim) {scrim.remove();scrim=null;}
  }

  function ensureBack(header) {
    if (back?.parentElement === header) return;
    removeBack();
    back = svgButton("relay-wa-back", "Back to chats", "M20 12H4m7-7-7 7 7 7");
    back.addEventListener("click", event => {
      event.preventDefault();
      event.stopPropagation();
      api.showList();
    });
    header.prepend(back);
  }

  function observe() {
    const root = document.getElementById("app") || document.body;
    if (root === observedRoot) return;
    observer.disconnect();
    observedRoot = root;
    if (root) observer.observe(root, { childList: true, subtree: true });
    // Detect replacing the application root as well as replacing its panes.
    if (root && root !== document.body && document.body) observer.observe(document.body, { childList: true });
  }

  function sync() {
    frame = 0;
    if (!enabled || !media.matches) {
      observer.disconnect();
      observedRoot = null;
      clearLayout();
      return;
    }
    if (document.visibilityState === "hidden") {
      observer.disconnect();
      observedRoot = null;
      return;
    }
    observe();
    const current = discover();
    if (!current) {
      clearLayout();
      return;
    }
    const ready = Boolean(current.main && current.header);
    if (!initialized) {
      wantChat = ready;
      initialized = true;
    }
    if (pendingSelection && ready) {
      wantChat = true;
      pendingSelection = false;
    }
    if (!ready) wantChat = false;
    layout = current;
    mark(current);
    const mode = wantChat && ready ? "chat" : "list";
    if (document.documentElement.getAttribute("data-relay-wa-pane") !== mode) {
      document.documentElement.setAttribute("data-relay-wa-pane", mode);
    }
    if (wantChat && ready) ensureBack(current.header);
    else removeBack();
    syncNavigation(current);
    if (focusList) {
      focusList = false;
      const target = lastRow?.isConnected ? lastRow : current.side.querySelector('input,[role="textbox"]');
      target?.focus({preventScroll:true});
    }
    if (focusChat && back) {focusChat=false;back.focus({preventScroll:true});}
  }

  function schedule() {
    if (enabled && media.matches && document.visibilityState !== "hidden" && !frame) frame = requestAnimationFrame(sync);
  }

  const observer = new MutationObserver(records => {
    const ownChanges = records.every(record => {
      const nodes = [...record.addedNodes, ...record.removedNodes];
      return nodes.length > 0 && nodes.every(node => node.nodeType === 1 && ["relay-wa-back","relay-wa-navigation","relay-wa-nav-scrim"].includes(node.id));
    });
    if (ownChanges) return;
    const relevant = records.some(record => {
      if (!layout || !layout.side.isConnected || (layout.main && !layout.main.isConnected)) return true;
      return !layout.pane.contains(record.target) && !layout.messages?.contains(record.target);
    });
    if (relevant) schedule();
  });

  function select(event) {
    if (!enabled || !media.matches || document.visibilityState === "hidden") return;
    if (event.type === "keydown" && event.key !== "Enter" && event.key !== " ") return;
    const target = event.target instanceof Element ? event.target : event.target?.parentElement;
    const row = target?.closest(rowSelector);
    const pane = document.getElementById("pane-side");
    if (!row || !pane?.contains(row)) return;
    const control = target.closest('button, [role="button"], [role="checkbox"], [role="switch"], input, select, textarea, a, [contenteditable="true"], [role="textbox"]');
    if (control && control !== row) return;
    lastRow = row;
    if (event.type === "keydown") focusChat = true;
    pendingSelection = true;
    schedule();
  }

  function geometry(node) {
    if (!node?.isConnected) return null;
    const rect = node.getBoundingClientRect();
    const round = value => Math.round(value * 100) / 100;
    return { x: round(rect.x), y: round(rect.y), width: round(rect.width), height: round(rect.height),
      visible: rect.width > 0 && rect.height > 0 && getComputedStyle(node).visibility !== "hidden" };
  }

  const api = {
    enable(value) {
      enabled = Boolean(value);
      if (enabled && !style) {
        style = document.createElement("style");
        style.id = "relay-wa-style";
        style.textContent = css;
        (document.head || document.documentElement).append(style);
      }
      if (frame) cancelAnimationFrame(frame);
      frame = 0;
      sync();
      if (!enabled) {
        initialized = false;
        pendingSelection = false;
        lastRow = null;
        style?.remove();
        style = null;
      }
      return null;
    },
    showList() {
      wantChat = false;
      initialized = true;
      pendingSelection = false;
      focusList = true;
      sync();
      return null;
    },
    showChat() {
      wantChat = true;
      initialized = true;
      pendingSelection = true;
      sync();
      return null;
    },
    status() {
      const current = layout || discover();
      const narrow = media.matches;
      return { enabled, narrow, recognized: Boolean(current),
        mode: enabled && narrow && layout ? (wantChat && layout.header ? "chat" : "list") : "wide",
        viewport: { width: innerWidth, height: innerHeight, client: document.documentElement.clientWidth,
          scroll: document.documentElement.scrollWidth },
        list: geometry(current?.side), chat: geometry(current?.main),
        back: Boolean(back?.isConnected), backGeometry: geometry(back) };
    }
  };

  document.addEventListener("click", select, true);
  document.addEventListener("keydown", select, true);
  document.addEventListener("keydown", event => {
    if (!enabled || !media.matches || event.key !== "Escape" || event.defaultPrevented || event.isComposing) return;
    const overlay = [...document.querySelectorAll('[role="dialog"],[role="menu"],[data-testid="drawer-right"]')].some(node => node.getClientRects().length);
    if (overlay) return;
    if (navigation) { navigation=false; sync(); navButton?.focus({preventScroll:true}); }
    else if (wantChat) api.showList();
    else return;
    event.preventDefault();
  });
  document.addEventListener("click", event => {
    if (navigation && layout?.nav?.contains(event.target)) {
      requestAnimationFrame(() => { navigation=false; sync(); });
    }
  }, true);
  document.addEventListener("visibilitychange", () => {
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
    if (document.visibilityState === "hidden") {
      observer.disconnect();
      observedRoot = null;
    } else if (enabled && media.matches) sync();
  });
  media.addEventListener("change", () => {
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
    sync();
  });
  globalThis.relayWhatsApp = api;
  return api;
})()
