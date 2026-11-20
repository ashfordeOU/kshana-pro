// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! Sector templates for the PNT dependency audit: the functions a sector usually has that
//! take position, navigation or time, so an inventory can be checked for what it leaves
//! out, and a starter inventory to fill in.
//!
//! Each template names the documents it is structured after. They are the criteria an audit
//! is measured against, not a claim that a template reproduces them or that an operator who
//! fills one in meets them.

/// One item a sector usually has.
pub struct Item {
    pub id: &'static str,
    pub name: &'static str,
    /// Other inventory ids that answer this item.
    pub aliases: &'static [&'static str],
    /// Why it matters for PNT.
    pub why: &'static str,
    /// The PNT kinds it usually needs.
    pub needs: &'static [&'static str],
    /// Whether it is usually a critical function.
    pub critical: bool,
}

/// A sector template.
pub struct Template {
    pub id: &'static str,
    pub title: &'static str,
    pub structured_after: &'static [&'static str],
    pub items: &'static [Item],
}

const fn it(
    id: &'static str,
    name: &'static str,
    aliases: &'static [&'static str],
    why: &'static str,
    needs: &'static [&'static str],
    critical: bool,
) -> Item {
    Item {
        id,
        name,
        aliases,
        why,
        needs,
        critical,
    }
}

const POS: &[&str] = &["position"];
const TIME: &[&str] = &["time"];
const POS_TIME: &[&str] = &["position", "time"];
const NAV: &[&str] = &["position", "velocity", "heading"];

pub const TEMPLATES: &[Template] = &[
    Template {
        id: "ship",
        title: "Ship (whole-ship GNSS dependency)",
        structured_after: &[
            "IACS UR E26 (inventory of computer-based systems)",
            "UK MCA MGN 719 (GNSS interference guidance and dependency mapping)",
            "IMO MSC.1/Circ.1575 (multi-system shipborne radionavigation receivers)",
        ],
        items: &[
            it("ecdis", "ECDIS", &["ecs", "chart-system"], "own-ship position and route monitoring", NAV, true),
            it("radar", "Radar / ARPA", &["arpa"], "ground-stabilised display and target vectors use GNSS speed and course", NAV, true),
            it("ais", "AIS transponder", &["ais-class-a"], "broadcasts GNSS position and takes UTC from it", POS_TIME, true),
            it("autopilot", "Autopilot / track control", &["track-control", "heading-control"], "track control steers to the GNSS position", NAV, true),
            it("gmdss-dsc", "GMDSS DSC radio", &["dsc", "vhf-dsc", "mf-hf-dsc"], "distress alerts carry the GNSS position", POS_TIME, true),
            it("vdr", "Voyage data recorder", &["s-vdr"], "records position and time for investigation", POS_TIME, false),
            it("satcom", "Satellite communications", &["vsat", "inmarsat", "leo-terminal"], "antenna pointing and timing use GNSS", POS_TIME, false),
            it("navtex", "NAVTEX / EGC receiver", &["egc", "safetynet"], "message area selection uses position", POS, false),
            it("bnwas-clock", "Bridge and ship clocks", &["master-clock", "ship-clock"], "clocks set from GNSS time", TIME, false),
            it("dp", "Dynamic positioning", &["dp-system"], "position reference systems often include GNSS", NAV, false),
            it("weather-routing", "Weather routing service", &["routing-service"], "routing advice from the reported position", POS, false),
            it("engine-logging", "Engine and alarm logging", &["ams", "alarm-monitoring"], "event timestamps from GNSS time", TIME, false),
        ],
    },
    Template {
        id: "port",
        title: "Port and terminal operations",
        structured_after: &[
            "UK port-sector GNSS reliance studies (operations dependency mapping)",
            "IACS UR E26 asset-inventory practice, applied ashore",
        ],
        items: &[
            it("vts", "Vessel traffic service", &["vtms"], "traffic picture from AIS positions and radar", POS_TIME, true),
            it("ais-base", "AIS base stations", &["ais-shore"], "timing and position of shore stations", POS_TIME, true),
            it("cranes", "Quay cranes and automation", &["sts-cranes", "crane-automation"], "positioning of spreaders and trolleys", POS, true),
            it("straddle-carriers", "Straddle carriers / AGVs", &["agv", "terminal-tractors"], "vehicle navigation and container location", POS, true),
            it("tos", "Terminal operating system", &["terminal-os"], "container location records", POS_TIME, true),
            it("gates", "Gate and OCR systems", &["gate-ocr"], "transaction timestamps", TIME, false),
            it("pilot-ppu", "Pilot portable units", &["ppu"], "pilotage position, often GNSS-RTK", NAV, true),
            it("berth-monitoring", "Berthing aid / mooring monitoring", &["berthing-aid"], "approach speed and distance", NAV, false),
            it("network-time", "Port network time (NTP/PTP)", &["ntp", "ptp"], "time for every IT and OT system", TIME, true),
            it("cctv", "CCTV and security recording", &["security-recording"], "evidential timestamps", TIME, false),
        ],
    },
    Template {
        id: "aircraft",
        title: "Aircraft architecture (per type)",
        structured_after: &[
            "Operator and pilot-association requests for transparency on GNSS dependencies in avionics",
            "EASA and ICAO safety information on GNSS interference",
        ],
        items: &[
            it("fms", "Flight management system", &["fmc"], "position blending and RNAV/RNP", NAV, true),
            it("irs", "Inertial reference system", &["adiru", "irs-ahrs"], "GNSS-aided inertial alignment and updating", NAV, true),
            it("ads-b-out", "ADS-B out / transponder", &["transponder", "ads-b"], "broadcast position and integrity figures", POS_TIME, true),
            it("taws", "TAWS / EGPWS", &["egpws"], "terrain alerting uses GNSS position", POS, true),
            it("approach", "SBAS/GBAS approach capability", &["lpv", "gls"], "approach guidance from GNSS", NAV, true),
            it("clock", "Aircraft clock / time source", &["cockpit-clock"], "time set from GNSS", TIME, false),
            it("elt", "Emergency locator transmitter", &["elt-ap"], "encoded position", POS, false),
            it("efb", "Electronic flight bag", &["efb-gnss"], "own-ship position on charts", POS, false),
            it("satcom-air", "Aircraft satcom", &["acars-satcom"], "antenna pointing and timing", POS_TIME, false),
        ],
    },
    Template {
        id: "water",
        title: "Water and wastewater utility",
        structured_after: &["UK critical-national-infrastructure timing dependency use cases (water)"],
        items: &[
            it("scada", "SCADA master", &["scada-master"], "event ordering and control from time-stamped telemetry", TIME, true),
            it("telemetry-outstations", "Telemetry outstations / RTUs", &["rtu", "outstations"], "local timestamps, often GNSS or network time", TIME, true),
            it("network-time", "Network time (NTP/PTP)", &["ntp", "ptp"], "time distribution to OT systems", TIME, true),
            it("plcs", "Treatment works PLCs", &["plc"], "sequence logging", TIME, false),
            it("metering", "Smart metering / AMI", &["ami"], "billing intervals", TIME, false),
            it("field-workforce", "Field workforce navigation", &["fleet-navigation"], "dispatch and asset location", POS, false),
            it("backhaul", "Telemetry backhaul (mobile network)", &["cellular-backhaul"], "mobile networks synchronise from GNSS", TIME, true),
        ],
    },
    Template {
        id: "pipeline",
        title: "Pipeline (oil, gas, chemicals)",
        structured_after: &["UK critical-national-infrastructure timing dependency use cases (energy, chemicals)"],
        items: &[
            it("leak-detection", "Leak detection (pressure-wave location)", &["lds"], "locating a leak needs synchronised timestamps along the line", TIME, true),
            it("scada", "Pipeline SCADA", &["scada-master"], "control and event ordering", TIME, true),
            it("rtus", "Block-valve RTUs", &["rtu", "valve-stations"], "local timestamps", TIME, true),
            it("network-time", "Network time (NTP/PTP)", &["ntp", "ptp"], "time distribution", TIME, true),
            it("metering", "Custody-transfer metering", &["fiscal-metering"], "flow integration intervals", TIME, false),
            it("sis", "Safety instrumented systems", &["esd"], "event sequence records", TIME, false),
        ],
    },
    Template {
        id: "emergency-calls",
        title: "Emergency call chain",
        structured_after: &["Emergency-call outage reviews (path timing dependency)", "EU-Alert / cell-broadcast practice"],
        items: &[
            it("call-routing", "Call routing and handling", &["psap-routing"], "routing and records depend on synchronised networks", TIME, true),
            it("caller-location", "Caller location (handset-derived)", &["aml", "advanced-mobile-location"], "location comes from the handset's GNSS", POS, true),
            it("cad", "Computer-aided dispatch", &["dispatch"], "incident and unit positions, timestamps", POS_TIME, true),
            it("avl", "Vehicle location (AVL)", &["fleet-tracking"], "unit positions from GNSS", POS, true),
            it("recording", "Call recording", &["voice-recording"], "evidential timestamps", TIME, false),
            it("network-time", "Network time (NTP/PTP)", &["ntp", "ptp"], "time distribution", TIME, true),
            it("cell-broadcast", "Public warning (cell broadcast)", &["eu-alert"], "mobile network timing", TIME, false),
        ],
    },
    Template {
        id: "tetra",
        title: "Public-safety radio (TETRA)",
        structured_after: &["TETRA base-station synchronisation practice"],
        items: &[
            it("base-stations", "Base stations", &["tbs"], "TDMA frame synchronisation, usually from GNSS", TIME, true),
            it("switch", "Switching and management", &["swmi"], "network timing and records", TIME, true),
            it("dispatch", "Dispatch consoles", &["control-room"], "unit locations and timestamps", POS_TIME, true),
            it("terminal-location", "Terminal location reporting", &["lip", "avl"], "GNSS positions of radios", POS, false),
            it("network-time", "Network time (NTP/PTP)", &["ntp", "ptp"], "time distribution", TIME, true),
        ],
    },
    Template {
        id: "rescue",
        title: "Rescue and ambulance service",
        structured_after: &["National rescue-service PNT dependency reviews", "Border-region responder navigation practice"],
        items: &[
            it("cad", "Computer-aided dispatch", &["dispatch"], "incident and unit positions", POS_TIME, true),
            it("vehicle-navigation", "Vehicle navigation", &["satnav", "mdt"], "route guidance from GNSS", NAV, true),
            it("avl", "Vehicle location (AVL)", &["fleet-tracking"], "unit positions from GNSS", POS, true),
            it("radio", "Radio network", &["tetra", "lmr"], "synchronisation from GNSS", TIME, true),
            it("air-rescue", "Air rescue navigation", &["hems", "sar-helicopter"], "GNSS approaches and navigation", NAV, false),
            it("paper-maps", "Paper maps and gazetteer", &["map-books"], "the fall-back when GNSS is lost", &[], false),
        ],
    },
];

/// The template ids.
pub fn ids() -> Vec<&'static str> {
    TEMPLATES.iter().map(|t| t.id).collect()
}

/// The template `id`.
pub fn find(id: &str) -> Result<&'static Template, String> {
    TEMPLATES
        .iter()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("unknown sector '{id}' (known: {})", ids().join(", ")))
}

/// A starter inventory for the template: one GNSS source, one non-GNSS source for each
/// PNT kind the template uses, and every item as a system needing its usual kinds from the
/// GNSS source. The operator edits it to match the installation.
pub fn starter(id: &str) -> Result<String, String> {
    let t = find(id)?;
    let mut s = format!(
        "# Starter PNT inventory for the '{}' template ({}).\n\
         # Structured after: {}.\n\
         # Edit every node to match the installation: providers in priority order, fallbacks,\n\
         # independent cross-checks, holdover of GNSS-disciplined clocks, isolation switches.\n\
         # Delete what the asset does not have; the audit lists template items left out.\n\
         format = \"{}\"\nasset = \"(asset name)\"\noperator = \"(operator)\"\nsector = \"{}\"\n\
         # required_autonomy_s = 86400\n\n\
         [[node]]\nid = \"gnss1\"\nname = \"GNSS receiver 1\"\ntype = \"gnss\"\n\
         provides = [\"position\", \"velocity\", \"time\"]\n# holdover_s = 0\n",
        t.id,
        t.title,
        t.structured_after.join("; "),
        super::INVENTORY_FORMAT,
        t.id
    );
    for it in t.items {
        s.push_str(&format!(
            "\n# {}: {}\n[[node]]\nid = \"{}\"\nname = \"{}\"\ntype = \"system\"\ncritical = {}\n# isolation_switch = true\n",
            it.name, it.why, it.id, it.name, it.critical
        ));
        for n in it.needs {
            s.push_str(&format!(
                "[[node.needs]]\npnt = \"{n}\"\nfrom = [\"gnss1\"]\n# fallback = []\n# cross_check = []\n"
            ));
        }
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_starter_parses_and_audits() {
        for id in ids() {
            let src = starter(id).unwrap();
            let inv = super::super::Inventory::parse(&src).unwrap_or_else(|e| panic!("{id}: {e}"));
            let a = super::super::audit(&inv, &src).unwrap();
            assert!(a.template_missing.is_empty(), "{id}");
            // Everything hangs off one GNSS receiver: level L1.
            assert_eq!(a.status.level, "L1", "{id}");
        }
        assert!(find("nope").is_err());
        let mut seen = std::collections::BTreeSet::new();
        for t in TEMPLATES {
            for i in t.items {
                assert!(seen.insert((t.id, i.id)), "{} {}", t.id, i.id);
            }
        }
    }
}
