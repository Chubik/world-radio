import { mountAccount } from "./views/account.js";

// this window is a temporary host for the account section; the main window's
// sidebar mounts the same module.
const account = mountAccount(document.getElementById("account_host"));

// the window is reused rather than recreated, so its state must be re-read each
// time the tray reopens it.
document.addEventListener("visibilitychange", () => {
  if (!document.hidden) account.refresh();
});
