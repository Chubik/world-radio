import { maskKey, accountStatus, normalizeKey, isValidKey } from "../labels.js";

const invoke = window.__TAURI__.core.invoke;

// the three states of one flow. "reveal" is the save-this screen and is the only
// place the full id is ever put on screen; it is held in a closure rather than in
// the dom so that leaving the screen drops the last reference to it.
const SIGNED_OUT = "signed_out";
const REVEAL = "reveal";
const SIGNED_IN = "signed_in";

// `onFilters` is how the sidebar hears that a sync or a sign-in brought a new
// country filter down from another device. without it the filter row would keep
// naming the old countries until the window was hidden and reopened.
export function mountAccount(host, onFilters = () => {}) {
  let view = SIGNED_OUT;
  let state = { signed_in: false, masked: "", favourites: 0 };
  let freshKey = null;
  let signingIn = false;
  let busy = false;

  function note(el, text, tone) {
    el.className = `note ${tone}`;
    el.textContent = text;
  }

  function el(tag, className, text) {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  }

  async function refresh() {
    // a create or sign-in in flight owns the view; re-rendering under it would
    // discard the pending call's buttons and its progress message.
    if (busy) {
      return;
    }
    try {
      state = await invoke("account_state");
    } catch {
      // a failure here must not strand the section on a stale signed-in view.
      state = { signed_in: false, masked: "", favourites: 0 };
    }
    render();
  }

  function renderSignedOut(root) {
    const card = el("div", "sc");
    card.appendChild(el("div", "tag", "Not signed in"));

    const lede = el("p", "lede");
    lede.append(
      "Sync your ",
      el("b", null, "★ favorites"),
      " across devices. No email, no password — just one anonymous ID."
    );
    card.appendChild(lede);

    const generate = el("button", "btn primary", "⚡ Generate account");
    card.appendChild(generate);

    const msg = el("p", "note dim", "");
    const prompt = el("div", "signin_prompt");
    const link = el("span", "link", "Sign in");
    prompt.append("Have an ID? ", link);

    const form = el("div", "signin_form");
    const field = el("input", "field");
    field.type = "password";
    field.spellcheck = false;
    field.autocomplete = "off";
    field.placeholder = "r4-…";
    const submit = el("button", "btn ghost", "Sign in");
    form.append(field, submit);
    form.hidden = !signingIn;
    prompt.hidden = signingIn;

    generate.addEventListener("click", async () => {
      if (busy) return;
      busy = true;
      note(msg, "creating your account…", "dim");
      generate.disabled = true;
      try {
        freshKey = await invoke("create_account");
        view = REVEAL;
        busy = false;
        render();
      } catch (e) {
        busy = false;
        note(msg, String(e), "err");
        generate.disabled = false;
      }
    });

    link.addEventListener("click", () => {
      signingIn = true;
      render();
    });

    async function doSignIn() {
      const key = normalizeKey(field.value);
      if (!isValidKey(key)) {
        note(msg, "that does not look like an r4dio ID", "err");
        return;
      }
      note(msg, "signing in…", "dim");
      submit.disabled = true;
      busy = true;
      try {
        await invoke("sign_in", { key });
        // the field held the full id; blank it before anything else can read it.
        field.value = "";
        signingIn = false;
        busy = false;
        await refresh();
        // signing in pulls the account's filter down with it.
        onFilters();
      } catch (e) {
        field.value = "";
        busy = false;
        note(msg, String(e), "err");
        submit.disabled = false;
      }
    }

    submit.addEventListener("click", doSignIn);
    field.addEventListener("keydown", (e) => {
      if (e.key === "Enter") doSignIn();
    });

    card.append(prompt, form, msg);
    root.appendChild(card);
  }

  function renderReveal(root) {
    const card = el("div", "sc");
    card.appendChild(el("div", "tag", "Save this — shown once"));

    const idbox = el("div", "idbox");
    idbox.appendChild(el("span", "k", freshKey));
    const copy = el("span", "cp", "⧉ copy");
    idbox.appendChild(copy);
    card.appendChild(idbox);

    const warn = el("p", "lede");
    warn.append(
      "Your ",
      el("b", null, "only key"),
      " — no email, no recovery. Closing this without saving means losing access."
    );
    card.appendChild(warn);

    const msg = el("p", "note dim", "");
    const done = el("button", "btn primary", "Done, I saved it");

    copy.addEventListener("click", async () => {
      try {
        await navigator.clipboard.writeText(freshKey);
        note(msg, "copied", "ok");
      } catch {
        note(msg, "could not copy — select the ID and copy it by hand", "err");
      }
    });

    done.addEventListener("click", async () => {
      // dropping the reference before re-rendering is what makes "shown once"
      // true: no later view can reach the full id.
      freshKey = null;
      view = SIGNED_IN;
      await refresh();
    });

    card.append(done, msg);
    root.appendChild(card);
  }

  async function drawQr(canvas) {
    let grid;
    try {
      grid = await invoke("account_qr");
    } catch {
      canvas.hidden = true;
      return;
    }
    // a failed call is caught above; a call that answers with nothing is a
    // different case, and reading .length off it would take the pane down.
    const n = grid?.length ?? 0;
    if (n === 0) {
      canvas.hidden = true;
      return;
    }
    const ctx = canvas.getContext("2d");
    const cell = canvas.width / n;
    ctx.fillStyle = "#F4ECD8";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.fillStyle = "#1a140c";
    for (let y = 0; y < n; y++) {
      for (let x = 0; x < n; x++) {
        if (grid[y][x]) ctx.fillRect(x * cell, y * cell, cell, cell);
      }
    }
  }

  function renderSignedIn(root) {
    const s = accountStatus(state);
    const card = el("div", "sc");
    card.appendChild(el("div", "tag", "Signed in"));
    card.appendChild(el("span", "synced-pill", s.pill));
    // maskKey guards the rendered value even though the backend already masked
    // it: the id reaches the screen through exactly one function.
    card.appendChild(el("div", "masked", maskKey(s.masked)));
    card.appendChild(el("p", "lede", `${s.detail} Add a device below.`));

    const canvas = el("canvas", "qrmini");
    canvas.width = 96;
    canvas.height = 96;
    card.appendChild(canvas);
    card.appendChild(el("div", "qrhint", "scan to sign in on another device"));

    const msg = el("p", "note dim", "");
    const now = el("button", "btn primary", "⟳ Sync now");
    const out = el("button", "btn ghost", "Sign out");
    const del = el("button", "btn danger", "Delete account");

    // the app follows the account live, but a device that was offline has
    // nothing to follow — this is how the user asks for the catch-up by hand.
    now.addEventListener("click", async () => {
      if (busy) return;
      busy = true;
      note(msg, "syncing…", "dim");
      now.disabled = true;
      try {
        await invoke("sync");
        note(msg, "synced", "ok");
      } catch (e) {
        note(msg, String(e), "err");
      }
      busy = false;
      now.disabled = false;
      // a sync can change the favourite count on this card and the country
      // filter the sidebar names, so both are re-read rather than just the card.
      try {
        state = await invoke("account_state");
      } catch {
        // a stale count is better than dropping the user out of a signed-in view.
      }
      onFilters();
    });

    out.addEventListener("click", async () => {
      try {
        await invoke("sign_out");
        await refresh();
      } catch (e) {
        note(msg, String(e), "err");
      }
    });

    del.addEventListener("click", async () => {
      if (!confirm("Delete your sync account on the server? Your favorites stay on this Mac.")) {
        return;
      }
      try {
        await invoke("delete_account");
        await refresh();
      } catch (e) {
        note(msg, String(e), "err");
      }
    });

    card.append(now, out, del, msg);
    root.appendChild(card);
    drawQr(canvas);
  }

  function render() {
    host.textContent = "";
    const root = el("div", "account");
    // the reveal screen owns the view until the user dismisses it, so a refresh
    // landing mid-flow cannot snatch the id away before it has been saved.
    if (view === REVEAL) {
      renderReveal(root);
      host.appendChild(root);
      return;
    }
    if (state.signed_in) {
      renderSignedIn(root);
      host.appendChild(root);
      return;
    }
    renderSignedOut(root);
    host.appendChild(root);
  }

  render();
  refresh();

  return { refresh };
}
