(() => {
  "use strict";
  if (window.top !== window || location.origin !== "https://web.whatsapp.com") return null;
  if (globalThis.relayWhatsApp) return globalThis.relayWhatsApp;

  const media = matchMedia("(max-width: 748px)");
  const attributes = ["data-relay-wa-path", "data-relay-wa-root", "data-relay-wa-list", "data-relay-wa-chat"];
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
  html[data-relay-wa-pane] [data-relay-wa-root] { position: relative !important; }
  html[data-relay-wa-pane] [data-relay-wa-list],
  html[data-relay-wa-pane] [data-relay-wa-chat] {
    position: absolute !important; inset: 0 !important; margin: 0 !important;
    flex: 1 1 100% !important; z-index: 2 !important;
  }
  html[data-relay-wa-pane="list"] [data-relay-wa-chat],
  html[data-relay-wa-pane="chat"] [data-relay-wa-list] { display: none !important; }
  #relay-wa-back {
    appearance: none; display: inline-grid; place-items: center; flex: 0 0 40px;
    width: 40px; height: 40px; padding: 8px; margin: 0 4px 0 0; order: -1;
    border: 0; border-radius: 0; background: transparent; color: inherit; cursor: pointer;
  }
  #relay-wa-back:focus-visible { outline: 2px solid currentColor; outline-offset: -3px; }
  #relay-wa-back svg { width: 24px; height: 24px; pointer-events: none; }
  [dir="rtl"] #relay-wa-back svg { transform: rotate(180deg); }
}`;

  function removeBack() {
    back?.remove();
    back = null;
  }

  function clearLayout() {
    document.documentElement.removeAttribute("data-relay-wa-pane");
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
    return { app, pane, side, main, root, listBranch, chatBranch,
      paths: [...new Set([...sidePath, ...mainPath, ...appPath])],
      header };
  }

  function mark(current) {
    const next = new Set(current.paths);
    for (const node of marked) {
      if (!next.has(node)) for (const name of attributes) node.removeAttribute(name);
    }
    for (const node of next) {
      const desired = [true, node === current.root, node === current.listBranch, node === current.chatBranch];
      attributes.forEach((name, index) => {
        if (node.hasAttribute(name) !== desired[index]) node.toggleAttribute(name, desired[index]);
      });
    }
    marked = next;
  }

  function ensureBack(header) {
    if (back?.parentElement === header) return;
    removeBack();
    back = document.createElement("button");
    back.id = "relay-wa-back";
    back.type = "button";
    back.title = "Back to chats";
    back.setAttribute("aria-label", "Back to chats");
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("viewBox", "0 0 24 24");
    svg.setAttribute("aria-hidden", "true");
    const arrow = document.createElementNS("http://www.w3.org/2000/svg", "path");
    arrow.setAttribute("d", "M20 12H4m7-7-7 7 7 7");
    arrow.setAttribute("fill", "none");
    arrow.setAttribute("stroke", "currentColor");
    arrow.setAttribute("stroke-width", "2");
    arrow.setAttribute("stroke-linecap", "round");
    arrow.setAttribute("stroke-linejoin", "round");
    svg.append(arrow);
    back.append(svg);
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
  }

  function schedule() {
    if (enabled && media.matches && document.visibilityState !== "hidden" && !frame) frame = requestAnimationFrame(sync);
  }

  const observer = new MutationObserver(records => {
    const ownChanges = records.every(record => {
      const nodes = [...record.addedNodes, ...record.removedNodes];
      return nodes.length > 0 && nodes.every(node => node.nodeType === 1 && node.id === "relay-wa-back");
    });
    if (!ownChanges) schedule();
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
        style?.remove();
        style = null;
      }
      return null;
    },
    showList() {
      wantChat = false;
      initialized = true;
      pendingSelection = false;
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
