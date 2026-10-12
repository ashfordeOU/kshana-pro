// SPDX-License-Identifier: AGPL-3.0-only
// The text edit that makes the Maritime trust view's geometry demo (tools_trust_geometry_demo.mjs) from the
// main synthetic demo log: the same edit as the engine's own test (tests/geometry_trust.rs, masked_with).
// Pure; tested in trust-geometry.test.mjs.

// NMEA checksum of a sentence body (between "$" and "*").
export function checksum(body) {
  let c = 0;
  for (let k = 0; k < body.length; k++) c ^= body.charCodeAt(k);
  return c.toString(16).toUpperCase().padStart(2, "0");
}

// From `fromS` seconds after the first GGA: every GSV satellite at azimuth 180 degrees or more loses its signal
// strength, the others drop by `dropDb`, and GGA reports HDOP `ggaHdop`. Checksums are recomputed.
export function maskedWith(log, fromS, dropDb, ggaHdop) {
  const out = [];
  let t = 0, t0 = null;
  for (const line of log.split("\n")) {
    if (!line) continue;
    const body = line.replace(/^\$/, "").split("*")[0];
    const f = body.split(",");
    if (f[0].endsWith("GGA")) {
      const s = f[1];
      const secs = Number(s.slice(0, 2)) * 3600 + Number(s.slice(2, 4)) * 60 + Number(s.slice(4));
      if (t0 === null) t0 = secs;
      t = secs - t0;
    }
    if (f[0].endsWith("GGA") && t >= fromS) {
      const g = f.slice(); g[8] = ggaHdop;
      const b = g.join(","); out.push(`$${b}*${checksum(b)}`);
    } else if (f[0].endsWith("GSV") && t >= fromS) {
      const g = f.slice();
      for (let i = 4; i + 3 < g.length; i += 4) {
        const az = Number(g[i + 2]) || 0;
        if (az >= 180) g[i + 3] = "";
        else if (g[i + 3] !== "" && Number.isFinite(Number(g[i + 3]))) g[i + 3] = (Number(g[i + 3]) - dropDb).toFixed(0);
      }
      const b = g.join(","); out.push(`$${b}*${checksum(b)}`);
    } else out.push(line);
  }
  return out.join("\n") + "\n";
}
