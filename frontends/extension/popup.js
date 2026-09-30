// BEXT-06: the setup popup's "Check Again" button asks the background to
// ping the native host; once it answers, the popup closes and the toolbar
// button sends pages again.
"use strict";

const api = globalThis.browser ?? globalThis.chrome;

const button = document.getElementById("check");
const status = document.getElementById("status");

button.addEventListener("click", async () => {
  button.disabled = true;
  status.textContent = "Checking…";
  try {
    const answer = await api.runtime.sendMessage({ type: "check-host" });
    if (answer?.installed) {
      window.close();
      return;
    }
    status.textContent = "Still not found.";
  } catch (error) {
    status.textContent = `Cannot check: ${error}`;
  } finally {
    button.disabled = false;
  }
});
