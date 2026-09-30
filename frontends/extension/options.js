// BEXT-03: "Close the tab after sending the page to Wye", off by default,
// kept in storage.local where background.js reads it.
"use strict";

const api = globalThis.browser ?? globalThis.chrome;

const OPTION_CLOSE_TAB = "closeTabAfterSending";
const box = document.getElementById(OPTION_CLOSE_TAB);

async function load() {
  const stored = await api.storage.local.get(OPTION_CLOSE_TAB);
  box.checked = stored[OPTION_CLOSE_TAB] === true;
}

box.addEventListener("change", () => {
  api.storage.local.set({ [OPTION_CLOSE_TAB]: box.checked });
});

load();
