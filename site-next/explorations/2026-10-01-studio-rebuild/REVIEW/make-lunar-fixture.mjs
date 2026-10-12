// Writes REVIEW/fixtures/lunar-station.synthetic.json: a made-up report in the engine's field names
// (src/lunar_station.rs) for the lunar station view's browser check. NOT an engine output and not ported (REVIEW/ is
// skipped). The numbers are an arbitrary smooth function of distance from the station, chosen to exercise the
// drawing: a threshold contour, an unavailable patch, and a coverage radius that varies with azimuth.
//   node REVIEW/make-lunar-fixture.mjs
import { writeFileSync, mkdirSync } from "node:fs";

const RM = 1737400, rad = (d) => (d * Math.PI) / 180, deg = (r) => (r * 180) / Math.PI;
const st = { lat: -89, lon: 0 };
const dist = (lat, lon) => { const a = Math.sin(rad(lat - st.lat) / 2) ** 2 + Math.cos(rad(lat)) * Math.cos(rad(st.lat)) * Math.sin(rad(lon - st.lon) / 2) ** 2; return (2 * RM * Math.asin(Math.sqrt(a))) / 1000; };
const bearing = (lat, lon) => (deg(Math.atan2(Math.sin(rad(lon - st.lon)) * Math.cos(rad(lat)), Math.cos(rad(st.lat)) * Math.sin(rad(lat)) - Math.sin(rad(st.lat)) * Math.cos(rad(lat)) * Math.cos(rad(lon - st.lon)))) + 360) % 360;
const value = (lat, lon) => { const d = dist(lat, lon), b = bearing(lat, lon); return 0.25 + d * (0.0035 + 0.0015 * Math.cos(rad(b - 60))); };
const points = [];
for (let lat = -90; lat <= -80; lat += 1) {
  const n = lat === -90 ? 1 : Math.round(360 * Math.cos(rad(lat)));
  for (let i = 0; i < n; i++) {
    const lon = lat === -90 ? 0 : -180 + (360 * (i + 0.5)) / n;
    const dead = lat >= -82 && lon > 100 && lon < 140;
    const v = value(lat, lon);
    points.push({ lat_deg: lat, lon_deg: Number(lon.toFixed(4)), distance_km: Number(dist(lat, lon).toFixed(3)), azimuth_deg: Number(bearing(lat, lon).toFixed(3)), area_weight: Math.cos(rad(lat)), availability: dead ? 0 : lat >= -83 && lon > 60 && lon < 160 ? 0.9 : 1,
      coverage_fraction: dead ? 0 : Number(Math.max(0, Math.min(1, 1.6 - v)).toFixed(4)), corrected_p_3d_m: dead ? null : Number(v.toFixed(4)), uncorrected_p_3d_m: dead ? null : Number((v * 18 + 6).toFixed(3)), hpl_median_m: dead ? null : Number((v * 4 + 1).toFixed(3)),
      unavailable: { user_lt4: dead ? 144 : 0, station_missing_sat: !dead && lat >= -83 && lon > 60 && lon < 160 ? 14 : 0 } });
  }
}
const live = points.filter((p) => p.corrected_p_3d_m != null), worst = live.reduce((a, b) => (b.corrected_p_3d_m > a.corrected_p_3d_m ? b : a));
const radius = Array.from({ length: 36 }, (_, k) => { const km = Number((0.75 / (0.0035 + 0.0015 * Math.cos(rad(k * 10 - 60)))).toFixed(1)); return { azimuth_deg: k * 10, radius_km: Math.min(km, 300), limited_by_grid: km >= 300, n_points: 10 }; });
const report = {
  kind: "lunar-station-coverage", label: "MODELLED", percentile: 95, n_sats: 8,
  error_model: { source: "orbit_and_clock", orbit_err_m: 100, clock_err_m: 30, los_sise_sigma_m: 40 },
  note: "Synthetic review fixture in the plan's field names; not an engine output. MODELLED; under stated public assumptions; not a statement about any real system; not affiliated.",
  inputs_echo: { station_lat_deg: -89, station_lon_deg: 0, station_height_m: 0, survey_sigma_m: 0.3, accuracy_threshold_m: 1, percentile: 95, alert_limit_m: 10, grid_lat_max_deg: -80, grid_lat_step_deg: 1, t_span_s: 86400, t_step_s: 600, n_trials: 200, seed: 42 },
  n_grid_points: points.length, n_epochs: 144, n_trials: 200,
  region: { availability: 0.962, coverage_fraction: 0.41, corrected_p_3d_m: 1.18, corrected_p_h_m: 0.74, corrected_p_v_m: 0.92, uncorrected_p_3d_m: 27.3, improvement_factor: 23.1, hpl_max_m: 11.9, vpl_max_m: 15.2, hpl_below_alert_fraction: 0.93, worst_point: { lat_deg: worst.lat_deg, lon_deg: worst.lon_deg, corrected_p_3d_m: worst.corrected_p_3d_m } },
  coverage_radius_km: radius, points,
  budget_at_station_and_at_edge: { epoch_s: 0, station: { orbit_m: 0.0, survey_m: 0.3, ageing_m: 0.05, quantization_m: 0.01, total_m: 0.31 }, worst_point: { orbit_m: 1.9, survey_m: 0.3, ageing_m: 0.05, quantization_m: 0.01, total_m: 1.93 } },
  units: { corrected_p_3d_m: "m", hpl_max_m: "m", survey_sigma_m: "m", accuracy_threshold_m: "m", t_step_s: "s" },
};
mkdirSync(new URL("./fixtures/", import.meta.url), { recursive: true });
writeFileSync(new URL("./fixtures/lunar-station.synthetic.json", import.meta.url), JSON.stringify(report));
// A wrapped copy, as a run_all reply, whose note names a programme: the page must withhold it.
writeFileSync(new URL("./fixtures/lunar-station.named-note.json", import.meta.url), JSON.stringify({ summary: "x", svg: "", json: JSON.stringify({ ...report, note: "A NovaMoon-class station (synthetic)." }) }));
console.log(`${points.length} points; worst ${worst.corrected_p_3d_m} m at ${worst.lat_deg}, ${worst.lon_deg}`);
