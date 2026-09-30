// Wye's browser extension (BEXT-01 to BEXT-06, IN-05): sends a link or the
// page to Wye through the native-messaging host `dev.soldunov.wye`
// (`wye-native-host`), which hands it to the Wye service.
//
// Shared by Firefox (event page) and Chromium (service worker). Every
// listener is registered at the top level so a restarted background still
// hears the event that woke it.
"use strict";

const api = globalThis.browser ?? globalThis.chrome;

const HOST = "dev.soldunov.wye";
const MENU_LINK = "wye-open-link";
const MENU_PAGE = "wye-open-page";
const COMMAND_PAGE = "open-page";
const SETUP_POPUP = "popup.html";
const ERROR_BADGE = "!";
// BEXT-03, stored in storage.local; off by default.
const OPTION_CLOSE_TAB = "closeTabAfterSending";

// BEXT-01: the context-menu entries.
async function createMenus() {
  await api.contextMenus.removeAll();
  api.contextMenus.create({
    id: MENU_LINK,
    title: "Open Link with Wye",
    contexts: ["link"],
  });
  api.contextMenus.create({
    id: MENU_PAGE,
    title: "Open Page with Wye",
    contexts: ["page"],
  });
}

// BEXT-05: the keys held during the click, as the browser names them
// (Firefox reports them; Chromium does not, so `null` lets Wye probe).
function heldKeys(clickData) {
  const modifiers = clickData?.modifiers;
  return Array.isArray(modifiers) ? modifiers : null;
}

// BEXT-06: without a host, the toolbar button opens the setup popup.
async function showHostState(installed) {
  await api.action.setPopup({ popup: installed ? "" : SETUP_POPUP });
  await api.action.setBadgeText({ text: installed ? "" : ERROR_BADGE });
}

async function showError(message) {
  await api.action.setBadgeText({ text: ERROR_BADGE });
  await api.action.setTitle({ title: `Wye: ${message}` });
}

async function clearError() {
  await api.action.setBadgeText({ text: "" });
  await api.action.setTitle({ title: "" });
}

async function closeTabAfterSending() {
  const stored = await api.storage.local.get(OPTION_CLOSE_TAB);
  return stored[OPTION_CLOSE_TAB] === true;
}

// Is the host installed? A ping never opens anything.
async function pingHost() {
  try {
    const reply = await api.runtime.sendNativeMessage(HOST, { ping: true });
    return reply?.ok === true;
  } catch {
    return false;
  }
}

async function checkHost() {
  await showHostState(await pingHost());
}

// Send `url` to Wye; `pageOrLink` is "page" or "link".
async function send(url, modifiers, pageOrLink, tab) {
  if (!url) {
    return;
  }
  let reply;
  try {
    reply = await api.runtime.sendNativeMessage(HOST, { url, modifiers, pageOrLink });
  } catch (error) {
    console.warn("Wye: the native host is missing or failed", error);
    await showHostState(false);
    return;
  }
  await showHostState(true);
  if (reply?.ok !== true) {
    await showError(reply?.error ?? "no answer from Wye");
    return;
  }
  await clearError();
  if (pageOrLink === "page" && tab?.id !== undefined && (await closeTabAfterSending())) {
    await api.tabs.remove(tab.id);
  }
}

async function activeTab() {
  const [tab] = await api.tabs.query({ active: true, currentWindow: true });
  return tab;
}

api.runtime.onInstalled.addListener(() => {
  createMenus();
  checkHost();
});

api.runtime.onStartup.addListener(() => {
  createMenus();
  checkHost();
});

api.contextMenus.onClicked.addListener((info, tab) => {
  if (info.menuItemId === MENU_LINK) {
    send(info.linkUrl, heldKeys(info), "link", tab);
  } else if (info.menuItemId === MENU_PAGE) {
    send(info.pageUrl ?? tab?.url, heldKeys(info), "page", tab);
  }
});

// BEXT-01: the toolbar button sends the page (only fires without a popup).
api.action.onClicked.addListener((tab, clickData) => {
  send(tab.url, heldKeys(clickData), "page", tab);
});

// BEXT-02: the browser-level shortcut. The keys of the shortcut itself are
// down, so they do not count as held.
api.commands.onCommand.addListener(async (command, tab) => {
  if (command !== COMMAND_PAGE) {
    return;
  }
  const target = tab ?? (await activeTab());
  send(target?.url, [], "page", target);
});

// The setup popup asks to check again after installing the host.
api.runtime.onMessage.addListener((message, _sender, sendResponse) => {
  if (message?.type !== "check-host") {
    return false;
  }
  pingHost().then(async (installed) => {
    await showHostState(installed);
    sendResponse({ installed });
  });
  return true;
});
