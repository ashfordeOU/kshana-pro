// SPDX-License-Identifier: AGPL-3.0-only
// Carries the synthetic OSNMA samples into tools/examples/osnma/ for the Tools page's OSNMA tab.
//
//   node tools_osnma_examples.mjs --repo <checkout holding examples/osnma>
//
// Copies, unchanged, the four inputs (plain pages and UBX-RXM-SFRBX, one good and one corrupted each), the
// expected `kshana osnma verify --json` output for each, and writes index.json with the throwaway public key
// the verifier is given. Nothing is a recording of any signal: the chain, key, data and tags are made up by
// the engine repository's generator (examples/osnma/README.md).
import { readFileSync, writeFileSync, copyFileSync, mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";
const i = process.argv.indexOf("--repo");
if (i < 0 || !process.argv[i + 1]) { console.error("usage: node tools_osnma_examples.mjs --repo <checkout>"); process.exit(2); }
const src = join(process.argv[i + 1], "examples", "osnma");
const here = new URL(".", import.meta.url).pathname;
const dst = join(here, "tools", "examples", "osnma");
rmSync(dst, { recursive: true, force: true }); mkdirSync(join(dst, "expected"), { recursive: true });
const key = readFileSync(join(src, "public-key.txt"), "utf8").trim();
const EX = [
  ["pages-good.txt", "Plain pages, a good chain: two satellites, every tag verifies. Synthetic."],
  ["pages-corrupt.txt", "The same pages with one flipped bit in one satellite's navigation data: the tag that covers it fails. Synthetic."],
  ["stream-good.ubx", "The good data as a u-blox UBX-RXM-SFRBX byte stream. Synthetic."],
  ["stream-corrupt.ubx", "The corrupted data as a UBX stream. Synthetic."],
];
for (const [f] of EX) { copyFileSync(join(src, f), join(dst, f)); copyFileSync(join(src, "expected", f + ".json"), join(dst, "expected", f + ".json")); }
writeFileSync(join(dst, "index.json"), JSON.stringify({
  note: "Synthetic samples made by the Kshana repository's generator; not a recording of any signal. The key is a throwaway.",
  public_key: key,
  examples: EX.map(([file, note]) => ({ file, expected: "expected/" + file + ".json", note })) }, null, 2) + "\n");
console.log("wrote tools/examples/osnma/ (" + EX.length + " examples, public key " + key.slice(0, 12) + "…)");
