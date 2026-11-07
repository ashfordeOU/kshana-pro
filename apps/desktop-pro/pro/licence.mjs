// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
// The licence page of Kshana Desktop Pro: status, offline activation, the machine ID for
// a bound licence, and the Upgrade offer, which is shown only while payments are on.
const invoke = (cmd, args) => window.__TAURI_INTERNALS__.invoke(cmd, args);
const $ = (id) => document.getElementById(id);
const date = (unix) => (unix ? new Date(unix * 1000).toISOString().slice(0, 10) : "never (perpetual)");

function row(label, value) {
  const dt = document.createElement("dt");
  dt.textContent = label;
  const dd = document.createElement("dd");
  dd.textContent = value;
  return [dt, dd];
}

async function show() {
  const s = await invoke("licence_status");
  const dl = $("status");
  dl.replaceChildren();
  const rows = [["State", s.state], ["Tier", s.tier]];
  if (s.licensee) rows.push(["Licensee", s.licensee], ["Expires", date(s.expires_unix)], ["Products", (s.products || []).join(", ")]);
  if (s.seats) rows.push(["Seats", String(s.seats)]);
  if (s.token_id) rows.push(["Token", s.token_id]);
  for (const [k, v] of rows) dl.append(...row(k, v));
  $("warning").hidden = !s.warning;
  $("warning").textContent = s.warning || "";
  const offer = await invoke("upgrade_offer", { edition: "pro-annual" });
  $("upgrade").hidden = !offer.offer;
  if (offer.offer) {
    $("offer").textContent = `${offer.name}: ${offer.price}`;
    $("checkout").href = offer.url;
  }
}

$("activate").addEventListener("click", async () => {
  const r = $("result");
  try {
    await invoke("licence_activate", { token: $("token").value.trim() });
    r.className = "";
    r.textContent = "Activated.";
    $("token").value = "";
  } catch (e) {
    r.className = "err";
    r.textContent = String(e);
  }
  await show();
});
$("deactivate").addEventListener("click", async () => {
  await invoke("licence_deactivate");
  $("result").textContent = "Removed.";
  await show();
});
invoke("licence_machine_id").then((m) => { $("mid").textContent = m || "not readable on this machine"; });
show();
