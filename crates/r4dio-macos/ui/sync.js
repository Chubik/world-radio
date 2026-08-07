import { normalizeKey, isValidKey, keyStatus, actionResult } from "./labels.js";

const invoke = window.__TAURI__.core.invoke;
const $ = (id) => document.getElementById(id);

let hasKey = false;

function showStatus() {
  const s = keyStatus(hasKey);
  $("status").className = `state ${s.tone}`;
  $("status_text").textContent = s.text;
  // the key itself is never rendered back, so the field starts empty every time.
  $("key").value = "";
  $("key").placeholder = hasKey ? "replace stored key" : "r4-…";
}

function showNote(action, ok) {
  const r = actionResult(action, ok);
  $("note").className = `note ${r.tone}`;
  $("note").textContent = r.text;
}

async function refresh() {
  hasKey = await invoke("has_sync_key");
  showStatus();
}

async function save() {
  const key = normalizeKey($("key").value);
  if (!isValidKey(key)) {
    showNote("save", false);
    return;
  }
  const ok = await invoke("set_sync_key", { key });
  showNote("save", ok);
  await refresh();
}

async function clear() {
  await invoke("clear_sync_key");
  showNote("clear", true);
  await refresh();
}

async function syncNow() {
  if (!hasKey) {
    showNote("sync", false);
    return;
  }
  await invoke("sync");
  showNote("sync", true);
}

$("save").addEventListener("click", save);
$("clear").addEventListener("click", clear);
$("sync_now").addEventListener("click", syncNow);
$("key").addEventListener("keydown", (e) => {
  if (e.key === "Enter") save();
});

// the window is reused rather than recreated, so its state must be re-read each
// time the tray reopens it.
document.addEventListener("visibilitychange", () => {
  if (!document.hidden) refresh();
});

refresh();
