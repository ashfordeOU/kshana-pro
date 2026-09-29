// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! ReqIF (Requirements Interchange Format, OMG formal/16-07-01, version 1.2) import
//! and export for the requirements model.
//!
//! **The subset read.** Every `SPEC-OBJECT` in `CORE-CONTENT/REQ-IF-CONTENT/SPEC-OBJECTS`
//! becomes one requirement, in document order. Its attribute values are resolved to
//! their attribute definitions by `IDENTIFIER`, and the definitions are matched by
//! `LONG-NAME` without regard to case, spaces, dots, hyphens or underscores:
//!
//! | Requirement field | Accepted `LONG-NAME`s |
//! |---|---|
//! | id | `ReqIF.ForeignID`, `ID`, `Identifier`, `ReqID` (else the SPEC-OBJECT `IDENTIFIER`) |
//! | text | `ReqIF.Text`, `Text`, `Description` |
//! | method | `Verification Method`, `Method`, `Verification` |
//! | criterion | `Acceptance Criterion`, `Criterion`, `Acceptance` |
//! | runs | `Kshana Runs` (`;`-separated) |
//! | capabilities | `Kshana Capabilities` (`;`-separated) |
//!
//! Value kinds read: `ATTRIBUTE-VALUE-STRING`, `-INTEGER`, `-REAL`, `-BOOLEAN`, `-DATE`
//! (the `THE-VALUE` attribute), `-XHTML` (the text content of `THE-VALUE`) and
//! `-ENUMERATION` (the `LONG-NAME` of each referenced `ENUM-VALUE`). Specification
//! hierarchies, relations, relation groups and tool extensions are not read.
//!
//! **The export** writes a complete ReqIF 1.2 document in the element order the ReqIF
//! schema fixes: a header, one string datatype, one enumeration datatype for the four
//! verification methods, one spec-object type with six attributes, one spec-object per
//! requirement and one specification listing them. Every `LAST-CHANGE` and the header
//! `CREATION-TIME` carry the stamp the caller passes, so the export is reproducible.

use super::requirements::{split_list, validate, Method, Requirement};
use super::xml::{self, escape, Element};
use kshana::api::KshanaError;

/// The ReqIF 1.2 namespace.
pub const REQIF_NS: &str = "http://www.omg.org/spec/ReqIF/20110401/reqif.xsd";

/// The stamp used when the caller gives none. It is a fixed value, not "now", so two
/// exports of the same requirements are byte-identical.
pub const DEFAULT_STAMP: &str = "1970-01-01T00:00:00Z";

fn err(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(format!("ReqIF: {}", msg.into()))
}

fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, ' ' | '.' | '-' | '_'))
        .flat_map(|c| c.to_lowercase())
        .collect()
}

#[derive(PartialEq)]
enum Field {
    Id,
    Text,
    Method,
    Criterion,
    Runs,
    Caps,
}

fn field_for(long_name: &str) -> Option<Field> {
    Some(match norm(long_name).as_str() {
        "reqifforeignid" | "id" | "identifier" | "reqid" => Field::Id,
        "reqiftext" | "text" | "description" => Field::Text,
        "verificationmethod" | "method" | "verification" => Field::Method,
        "acceptancecriterion" | "criterion" | "acceptance" => Field::Criterion,
        "kshanaruns" => Field::Runs,
        "kshanacapabilities" => Field::Caps,
        _ => return None,
    })
}

/// Import requirements from a ReqIF document.
pub fn from_reqif(src: &str) -> Result<Vec<Requirement>, KshanaError> {
    let root = xml::parse(src)?;
    if root.local_name() != "REQ-IF" {
        return Err(err(format!(
            "root element is <{}>, not <REQ-IF>",
            root.name
        )));
    }
    let content = root
        .child("CORE-CONTENT")
        .and_then(|c| c.child("REQ-IF-CONTENT"))
        .ok_or_else(|| err("no CORE-CONTENT/REQ-IF-CONTENT"))?;

    // Attribute definitions and enum values, by IDENTIFIER.
    let mut defs: Vec<(String, String)> = Vec::new();
    for kind in [
        "ATTRIBUTE-DEFINITION-STRING",
        "ATTRIBUTE-DEFINITION-XHTML",
        "ATTRIBUTE-DEFINITION-ENUMERATION",
        "ATTRIBUTE-DEFINITION-INTEGER",
        "ATTRIBUTE-DEFINITION-REAL",
        "ATTRIBUTE-DEFINITION-BOOLEAN",
        "ATTRIBUTE-DEFINITION-DATE",
    ] {
        let mut v = Vec::new();
        content.descendants(kind, &mut v);
        for d in v {
            if let (Some(id), Some(ln)) = (d.attr("IDENTIFIER"), d.attr("LONG-NAME")) {
                defs.push((id.to_string(), ln.to_string()));
            }
        }
    }
    let mut enum_vals = Vec::new();
    content.descendants("ENUM-VALUE", &mut enum_vals);
    let enum_name = |id: &str| -> Option<String> {
        enum_vals
            .iter()
            .find(|e| e.attr("IDENTIFIER") == Some(id))
            .and_then(|e| e.attr("LONG-NAME").map(str::to_string))
    };
    let def_name =
        |id: &str| -> Option<&str> { defs.iter().find(|(i, _)| i == id).map(|(_, n)| n.as_str()) };

    let objects = content
        .child("SPEC-OBJECTS")
        .ok_or_else(|| err("no SPEC-OBJECTS"))?;
    let mut out = Vec::new();
    for so in objects
        .elements()
        .filter(|e| e.local_name() == "SPEC-OBJECT")
    {
        let so_id = so.attr("IDENTIFIER").unwrap_or("").to_string();
        let (mut id, mut text, mut method, mut crit, mut runs, mut caps) =
            (None, String::new(), None, None, Vec::new(), Vec::new());
        if let Some(values) = so.child("VALUES") {
            for av in values.elements() {
                let def_ref = av
                    .child("DEFINITION")
                    .and_then(|d| d.elements().next())
                    .map(Element::text)
                    .unwrap_or_default();
                let Some(long_name) = def_name(def_ref.trim()) else {
                    continue;
                };
                let Some(field) = field_for(long_name) else {
                    continue;
                };
                let value = match av.local_name() {
                    "ATTRIBUTE-VALUE-XHTML" => {
                        av.child("THE-VALUE").map(Element::text).unwrap_or_default()
                    }
                    "ATTRIBUTE-VALUE-ENUMERATION" => {
                        let mut refs = Vec::new();
                        if let Some(vs) = av.child("VALUES") {
                            vs.descendants("ENUM-VALUE-REF", &mut refs);
                        }
                        refs.iter()
                            .filter_map(|r| enum_name(r.text().trim()))
                            .collect::<Vec<_>>()
                            .join(";")
                    }
                    _ => av.attr("THE-VALUE").unwrap_or("").to_string(),
                };
                match field {
                    Field::Id => id = Some(value.trim().to_string()),
                    Field::Text => text = value.trim().to_string(),
                    Field::Method => method = Some(value),
                    Field::Criterion => {
                        crit = Some(value.trim().to_string()).filter(|s| !s.is_empty())
                    }
                    Field::Runs => runs = split_list(&value),
                    Field::Caps => caps = split_list(&value),
                }
            }
        }
        let id = id.filter(|s| !s.is_empty()).unwrap_or(so_id.clone());
        let m = method.ok_or_else(|| {
            err(format!(
                "SPEC-OBJECT '{so_id}' has no verification-method attribute"
            ))
        })?;
        let method = Method::parse(&m).ok_or_else(|| {
            err(format!(
                "SPEC-OBJECT '{so_id}': method '{m}' is not test, analysis, inspection or demonstration"
            ))
        })?;
        out.push(Requirement {
            id,
            text,
            method,
            criterion: crit,
            runs,
            capabilities: caps,
        });
    }
    validate(&out)?;
    Ok(out)
}

/// An `xsd:ID`-safe identifier (an XML NCName) derived from a requirement id.
fn ncname(prefix: &str, s: &str) -> String {
    let body: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{prefix}{body}")
}

/// Export requirements as a ReqIF 1.2 document.
pub fn to_reqif(title: &str, reqs: &[Requirement], stamp: Option<&str>) -> String {
    let lc = escape(stamp.unwrap_or(DEFAULT_STAMP));
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    s.push_str(&format!(
        "<REQ-IF xmlns=\"{REQIF_NS}\" xmlns:xhtml=\"http://www.w3.org/1999/xhtml\">\n"
    ));
    s.push_str("  <THE-HEADER>\n    <REQ-IF-HEADER IDENTIFIER=\"KP-HEADER\">\n");
    s.push_str("      <COMMENT>Exported by Kshana Pro MBSE. Acceptance criteria are machine-checkable expressions over open Kshana engine result documents.</COMMENT>\n");
    s.push_str(&format!("      <CREATION-TIME>{lc}</CREATION-TIME>\n"));
    s.push_str("      <REQ-IF-TOOL-ID>Kshana Pro</REQ-IF-TOOL-ID>\n");
    s.push_str("      <REQ-IF-VERSION>1.0</REQ-IF-VERSION>\n");
    s.push_str("      <SOURCE-TOOL-ID>Kshana Pro MBSE</SOURCE-TOOL-ID>\n");
    s.push_str(&format!("      <TITLE>{}</TITLE>\n", escape(title)));
    s.push_str("    </REQ-IF-HEADER>\n  </THE-HEADER>\n");
    s.push_str("  <CORE-CONTENT>\n    <REQ-IF-CONTENT>\n      <DATATYPES>\n");
    s.push_str(&format!("        <DATATYPE-DEFINITION-STRING IDENTIFIER=\"KP-DT-STRING\" LAST-CHANGE=\"{lc}\" LONG-NAME=\"String\" MAX-LENGTH=\"32000\"/>\n"));
    s.push_str(&format!("        <DATATYPE-DEFINITION-ENUMERATION IDENTIFIER=\"KP-DT-METHOD\" LAST-CHANGE=\"{lc}\" LONG-NAME=\"Verification method\">\n          <SPECIFIED-VALUES>\n"));
    for (i, m) in ["test", "analysis", "inspection", "demonstration"]
        .iter()
        .enumerate()
    {
        s.push_str(&format!(
            "            <ENUM-VALUE IDENTIFIER=\"KP-EV-{}\" LAST-CHANGE=\"{lc}\" LONG-NAME=\"{m}\">\n              <PROPERTIES><EMBEDDED-VALUE KEY=\"{i}\" OTHER-CONTENT=\"\"/></PROPERTIES>\n            </ENUM-VALUE>\n",
            m.to_ascii_uppercase()
        ));
    }
    s.push_str("          </SPECIFIED-VALUES>\n        </DATATYPE-DEFINITION-ENUMERATION>\n      </DATATYPES>\n");
    s.push_str("      <SPEC-TYPES>\n");
    s.push_str(&format!("        <SPEC-OBJECT-TYPE IDENTIFIER=\"KP-SOT-REQ\" LAST-CHANGE=\"{lc}\" LONG-NAME=\"Requirement\">\n          <SPEC-ATTRIBUTES>\n"));
    let string_def = |id: &str, name: &str| {
        format!("            <ATTRIBUTE-DEFINITION-STRING IDENTIFIER=\"{id}\" LAST-CHANGE=\"{lc}\" LONG-NAME=\"{name}\">\n              <TYPE><DATATYPE-DEFINITION-STRING-REF>KP-DT-STRING</DATATYPE-DEFINITION-STRING-REF></TYPE>\n            </ATTRIBUTE-DEFINITION-STRING>\n")
    };
    s.push_str(&string_def("KP-AD-ID", "ReqIF.ForeignID"));
    s.push_str(&string_def("KP-AD-TEXT", "ReqIF.Text"));
    s.push_str(&format!("            <ATTRIBUTE-DEFINITION-ENUMERATION IDENTIFIER=\"KP-AD-METHOD\" LAST-CHANGE=\"{lc}\" LONG-NAME=\"Verification Method\" MULTI-VALUED=\"false\">\n              <TYPE><DATATYPE-DEFINITION-ENUMERATION-REF>KP-DT-METHOD</DATATYPE-DEFINITION-ENUMERATION-REF></TYPE>\n            </ATTRIBUTE-DEFINITION-ENUMERATION>\n"));
    s.push_str(&string_def("KP-AD-CRITERION", "Acceptance Criterion"));
    s.push_str(&string_def("KP-AD-RUNS", "Kshana Runs"));
    s.push_str(&string_def("KP-AD-CAPS", "Kshana Capabilities"));
    s.push_str("          </SPEC-ATTRIBUTES>\n        </SPEC-OBJECT-TYPE>\n");
    s.push_str(&format!("        <SPECIFICATION-TYPE IDENTIFIER=\"KP-ST\" LAST-CHANGE=\"{lc}\" LONG-NAME=\"Requirements specification\"/>\n"));
    s.push_str("      </SPEC-TYPES>\n      <SPEC-OBJECTS>\n");
    let mut ids: Vec<String> = Vec::new();
    for (i, r) in reqs.iter().enumerate() {
        let mut so_id = ncname("KP-SO-", &r.id);
        if ids.contains(&so_id) {
            so_id = format!("{so_id}-{i}");
        }
        ids.push(so_id.clone());
        let sv = |def: &str, v: &str| {
            format!("            <ATTRIBUTE-VALUE-STRING THE-VALUE=\"{}\"><DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>{def}</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION></ATTRIBUTE-VALUE-STRING>\n", escape(v))
        };
        s.push_str(&format!(
            "        <SPEC-OBJECT IDENTIFIER=\"{so_id}\" LAST-CHANGE=\"{lc}\">\n          <VALUES>\n"
        ));
        s.push_str(&sv("KP-AD-ID", &r.id));
        s.push_str(&sv("KP-AD-TEXT", &r.text));
        s.push_str(&format!("            <ATTRIBUTE-VALUE-ENUMERATION><DEFINITION><ATTRIBUTE-DEFINITION-ENUMERATION-REF>KP-AD-METHOD</ATTRIBUTE-DEFINITION-ENUMERATION-REF></DEFINITION><VALUES><ENUM-VALUE-REF>KP-EV-{}</ENUM-VALUE-REF></VALUES></ATTRIBUTE-VALUE-ENUMERATION>\n", r.method.as_str().to_ascii_uppercase()));
        if let Some(c) = &r.criterion {
            s.push_str(&sv("KP-AD-CRITERION", c));
        }
        if !r.runs.is_empty() {
            s.push_str(&sv("KP-AD-RUNS", &r.runs.join(";")));
        }
        if !r.capabilities.is_empty() {
            s.push_str(&sv("KP-AD-CAPS", &r.capabilities.join(";")));
        }
        s.push_str("          </VALUES>\n          <TYPE><SPEC-OBJECT-TYPE-REF>KP-SOT-REQ</SPEC-OBJECT-TYPE-REF></TYPE>\n        </SPEC-OBJECT>\n");
    }
    s.push_str("      </SPEC-OBJECTS>\n      <SPECIFICATIONS>\n");
    s.push_str(&format!("        <SPECIFICATION IDENTIFIER=\"KP-SPEC\" LAST-CHANGE=\"{lc}\" LONG-NAME=\"{}\">\n          <TYPE><SPECIFICATION-TYPE-REF>KP-ST</SPECIFICATION-TYPE-REF></TYPE>\n          <CHILDREN>\n", escape(title)));
    for so_id in &ids {
        s.push_str(&format!("            <SPEC-HIERARCHY IDENTIFIER=\"KP-SH-{}\" LAST-CHANGE=\"{lc}\"><OBJECT><SPEC-OBJECT-REF>{so_id}</SPEC-OBJECT-REF></OBJECT></SPEC-HIERARCHY>\n", &so_id[6..]));
    }
    s.push_str("          </CHILDREN>\n        </SPECIFICATION>\n      </SPECIFICATIONS>\n");
    s.push_str("    </REQ-IF-CONTENT>\n  </CORE-CONTENT>\n</REQ-IF>\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reqs() -> Vec<Requirement> {
        vec![
            Requirement {
                id: "CLK-001".into(),
                text: "Timing error < 20 ns & \"stable\"".into(),
                method: Method::Test,
                criterion: Some("classical.fom.timing_p95_ns <= 20 ns".into()),
                runs: vec!["RUN-CLOCK".into()],
                capabilities: vec![],
            },
            Requirement {
                id: "INS 2".into(),
                text: "Label present".into(),
                method: Method::Inspection,
                criterion: None,
                runs: vec![],
                capabilities: vec!["GNSS-denied clock holdover".into()],
            },
        ]
    }

    #[test]
    fn round_trip_is_lossless_and_deterministic() {
        let a = to_reqif("SRD", &reqs(), None);
        assert_eq!(a, to_reqif("SRD", &reqs(), None));
        assert_eq!(from_reqif(&a).unwrap(), reqs());
        assert!(a.contains("IDENTIFIER=\"KP-SO-INS_2\""));
    }

    #[test]
    fn reads_xhtml_text_and_enumerations_from_another_tool() {
        // Shape of a DOORS/Polarion-style export: XHTML text, enumeration method,
        // foreign attribute names, extra unknown attributes.
        let doc = r#"<?xml version="1.0" encoding="UTF-8"?>
<REQ-IF xmlns="http://www.omg.org/spec/ReqIF/20110401/reqif.xsd" xmlns:xhtml="http://www.w3.org/1999/xhtml">
 <THE-HEADER><REQ-IF-HEADER IDENTIFIER="h"><CREATION-TIME>2026-01-01T00:00:00Z</CREATION-TIME><REQ-IF-TOOL-ID>x</REQ-IF-TOOL-ID><REQ-IF-VERSION>1.0</REQ-IF-VERSION><SOURCE-TOOL-ID>x</SOURCE-TOOL-ID><TITLE>t</TITLE></REQ-IF-HEADER></THE-HEADER>
 <CORE-CONTENT><REQ-IF-CONTENT>
  <DATATYPES><DATATYPE-DEFINITION-ENUMERATION IDENTIFIER="dt" LAST-CHANGE="x" LONG-NAME="VM"><SPECIFIED-VALUES>
    <ENUM-VALUE IDENTIFIER="ev-a" LAST-CHANGE="x" LONG-NAME="Analysis"><PROPERTIES><EMBEDDED-VALUE KEY="1" OTHER-CONTENT=""/></PROPERTIES></ENUM-VALUE>
  </SPECIFIED-VALUES></DATATYPE-DEFINITION-ENUMERATION></DATATYPES>
  <SPEC-TYPES><SPEC-OBJECT-TYPE IDENTIFIER="t" LAST-CHANGE="x" LONG-NAME="Req"><SPEC-ATTRIBUTES>
   <ATTRIBUTE-DEFINITION-STRING IDENTIFIER="a1" LAST-CHANGE="x" LONG-NAME="ID"/>
   <ATTRIBUTE-DEFINITION-XHTML IDENTIFIER="a2" LAST-CHANGE="x" LONG-NAME="ReqIF.Text"/>
   <ATTRIBUTE-DEFINITION-ENUMERATION IDENTIFIER="a3" LAST-CHANGE="x" LONG-NAME="Verification_Method" MULTI-VALUED="false"/>
   <ATTRIBUTE-DEFINITION-STRING IDENTIFIER="a4" LAST-CHANGE="x" LONG-NAME="acceptance criterion"/>
   <ATTRIBUTE-DEFINITION-STRING IDENTIFIER="a5" LAST-CHANGE="x" LONG-NAME="Owner"/>
  </SPEC-ATTRIBUTES></SPEC-OBJECT-TYPE></SPEC-TYPES>
  <SPEC-OBJECTS><SPEC-OBJECT IDENTIFIER="so1" LAST-CHANGE="x"><VALUES>
   <ATTRIBUTE-VALUE-STRING THE-VALUE="SYS-9"><DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>a1</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION></ATTRIBUTE-VALUE-STRING>
   <ATTRIBUTE-VALUE-XHTML><DEFINITION><ATTRIBUTE-DEFINITION-XHTML-REF>a2</ATTRIBUTE-DEFINITION-XHTML-REF></DEFINITION><THE-VALUE><xhtml:div>The <xhtml:b>holdover</xhtml:b> shall last.</xhtml:div></THE-VALUE></ATTRIBUTE-VALUE-XHTML>
   <ATTRIBUTE-VALUE-ENUMERATION><DEFINITION><ATTRIBUTE-DEFINITION-ENUMERATION-REF>a3</ATTRIBUTE-DEFINITION-ENUMERATION-REF></DEFINITION><VALUES><ENUM-VALUE-REF>ev-a</ENUM-VALUE-REF></VALUES></ATTRIBUTE-VALUE-ENUMERATION>
   <ATTRIBUTE-VALUE-STRING THE-VALUE="classical.fom.holdover_s &gt;= 30 min"><DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>a4</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION></ATTRIBUTE-VALUE-STRING>
   <ATTRIBUTE-VALUE-STRING THE-VALUE="someone"><DEFINITION><ATTRIBUTE-DEFINITION-STRING-REF>a5</ATTRIBUTE-DEFINITION-STRING-REF></DEFINITION></ATTRIBUTE-VALUE-STRING>
  </VALUES><TYPE><SPEC-OBJECT-TYPE-REF>t</SPEC-OBJECT-TYPE-REF></TYPE></SPEC-OBJECT></SPEC-OBJECTS>
 </REQ-IF-CONTENT></CORE-CONTENT></REQ-IF>"#;
        let r = from_reqif(doc).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "SYS-9");
        assert_eq!(r[0].text, "The holdover shall last.");
        assert_eq!(r[0].method, Method::Analysis);
        assert_eq!(
            r[0].criterion.as_deref(),
            Some("classical.fom.holdover_s >= 30 min")
        );
    }

    #[test]
    fn rejects_non_reqif_and_missing_method() {
        assert!(from_reqif("<foo/>").is_err());
        let a = to_reqif("SRD", &reqs(), None);
        let stripped = a.replace(
            "KP-AD-METHOD</ATTRIBUTE-DEFINITION-ENUMERATION-REF>",
            "NOPE</ATTRIBUTE-DEFINITION-ENUMERATION-REF>",
        );
        assert!(from_reqif(&stripped).is_err());
    }
}
