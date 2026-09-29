// PROPRIETARY — Kshana Pro overlay. Not part of the open-source engine. See LICENSE.
//
//! A small, strict XML reader for the ReqIF subset. It reads elements, attributes,
//! text, CDATA sections, comments, processing instructions and a DOCTYPE line, and
//! decodes the five predefined entities and numeric character references. It does not
//! validate against a schema, expand external entities or read a DTD, by design: a
//! requirements file is data, and nothing in it is fetched or executed.

use kshana::api::KshanaError;

/// One XML element.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    /// The name as written, including any namespace prefix.
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

/// A child node.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Element(Element),
    Text(String),
}

impl Element {
    /// The name without its namespace prefix.
    pub fn local_name(&self) -> &str {
        self.name.rsplit(':').next().unwrap_or(&self.name)
    }
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }
    /// The first child element with this local name.
    pub fn child(&self, local: &str) -> Option<&Element> {
        self.elements().find(|e| e.local_name() == local)
    }
    /// Every descendant element (depth first, document order) with this local name.
    pub fn descendants<'a>(&'a self, local: &str, out: &mut Vec<&'a Element>) {
        for e in self.elements() {
            if e.local_name() == local {
                out.push(e);
            }
            e.descendants(local, out);
        }
    }
    /// All text inside this element, concatenated, with whitespace runs collapsed.
    pub fn text(&self) -> String {
        fn collect(e: &Element, out: &mut String) {
            for n in &e.children {
                match n {
                    Node::Text(t) => out.push_str(t),
                    Node::Element(c) => {
                        collect(c, out);
                        out.push(' ');
                    }
                }
            }
        }
        let mut s = String::new();
        collect(self, &mut s);
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

fn err(msg: impl Into<String>) -> KshanaError {
    KshanaError::InvalidInput(format!("XML: {}", msg.into()))
}

/// Decode entity and character references.
pub fn decode(s: &str) -> Result<String, KshanaError> {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let end = after
            .find(';')
            .ok_or_else(|| err("an '&' is not followed by an entity reference"))?;
        let ent = &after[..end];
        let ch = match ent {
            "lt" => '<',
            "gt" => '>',
            "amp" => '&',
            "quot" => '"',
            "apos" => '\'',
            _ if ent.starts_with("#x") || ent.starts_with("#X") => char::from_u32(
                u32::from_str_radix(&ent[2..], 16)
                    .map_err(|_| err(format!("bad reference &{ent};")))?,
            )
            .ok_or_else(|| err(format!("bad reference &{ent};")))?,
            _ if ent.starts_with('#') => char::from_u32(
                ent[1..]
                    .parse::<u32>()
                    .map_err(|_| err(format!("bad reference &{ent};")))?,
            )
            .ok_or_else(|| err(format!("bad reference &{ent};")))?,
            _ => return Err(err(format!("unknown entity &{ent};"))),
        };
        out.push(ch);
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(out)
}

/// Escape text for an attribute value or element content.
pub fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\n' => o.push_str("&#10;"),
            '\r' => o.push_str("&#13;"),
            '\t' => o.push_str("&#9;"),
            _ => o.push(c),
        }
    }
    o
}

struct Reader<'a> {
    s: &'a str,
    pos: usize,
}

impl<'a> Reader<'a> {
    fn rest(&self) -> &'a str {
        &self.s[self.pos..]
    }
    fn skip_ws(&mut self) {
        let r = self.rest();
        self.pos += r.len() - r.trim_start().len();
    }
    fn skip_past(&mut self, pat: &str, what: &str) -> Result<(), KshanaError> {
        let i = self
            .rest()
            .find(pat)
            .ok_or_else(|| err(format!("unterminated {what}")))?;
        self.pos += i + pat.len();
        Ok(())
    }
    fn name(&mut self) -> Result<String, KshanaError> {
        let r = self.rest();
        let n = r
            .char_indices()
            .find(|(_, c)| !(c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | ':')))
            .map(|(i, _)| i)
            .unwrap_or(r.len());
        if n == 0 {
            return Err(err(format!("expected a name at byte {}", self.pos)));
        }
        self.pos += n;
        Ok(r[..n].to_string())
    }
    fn misc(&mut self) -> Result<(), KshanaError> {
        loop {
            self.skip_ws();
            let r = self.rest();
            if r.starts_with("<?") {
                self.skip_past("?>", "processing instruction")?;
            } else if r.starts_with("<!--") {
                self.skip_past("-->", "comment")?;
            } else if r.starts_with("<!DOCTYPE") {
                if r[..r.find('>').unwrap_or(r.len())].contains('[') {
                    return Err(err("an internal DTD subset is not accepted"));
                }
                self.skip_past(">", "DOCTYPE")?;
            } else {
                return Ok(());
            }
        }
    }
    fn element(&mut self, depth: usize) -> Result<Element, KshanaError> {
        if depth > 256 {
            return Err(err("elements nested more than 256 deep"));
        }
        if !self.rest().starts_with('<') {
            return Err(err(format!("expected '<' at byte {}", self.pos)));
        }
        self.pos += 1;
        let name = self.name()?;
        let mut attrs = Vec::new();
        loop {
            self.skip_ws();
            let r = self.rest();
            if r.starts_with("/>") {
                self.pos += 2;
                return Ok(Element {
                    name,
                    attrs,
                    children: vec![],
                });
            }
            if r.starts_with('>') {
                self.pos += 1;
                break;
            }
            let an = self.name()?;
            self.skip_ws();
            if !self.rest().starts_with('=') {
                return Err(err(format!("attribute '{an}' has no value")));
            }
            self.pos += 1;
            self.skip_ws();
            let q = self
                .rest()
                .chars()
                .next()
                .filter(|c| *c == '"' || *c == '\'')
                .ok_or_else(|| err(format!("attribute '{an}' value is not quoted")))?;
            self.pos += 1;
            let end = self
                .rest()
                .find(q)
                .ok_or_else(|| err(format!("attribute '{an}' value is unterminated")))?;
            let raw = &self.rest()[..end];
            if raw.contains('<') {
                return Err(err(format!("'<' inside attribute '{an}'")));
            }
            attrs.push((an, decode(raw)?));
            self.pos += end + 1;
        }
        let mut children = Vec::new();
        loop {
            let r = self.rest();
            if r.is_empty() {
                return Err(err(format!("element <{name}> is not closed")));
            }
            if r.starts_with("</") {
                self.pos += 2;
                let close = self.name()?;
                if close != name {
                    return Err(err(format!("</{close}> closes <{name}>")));
                }
                self.skip_ws();
                if !self.rest().starts_with('>') {
                    return Err(err(format!("malformed </{close}>")));
                }
                self.pos += 1;
                return Ok(Element {
                    name,
                    attrs,
                    children,
                });
            } else if r.starts_with("<!--") {
                self.skip_past("-->", "comment")?;
            } else if r.starts_with("<![CDATA[") {
                self.pos += 9;
                let end = self
                    .rest()
                    .find("]]>")
                    .ok_or_else(|| err("unterminated CDATA section"))?;
                children.push(Node::Text(self.rest()[..end].to_string()));
                self.pos += end + 3;
            } else if r.starts_with("<?") {
                self.skip_past("?>", "processing instruction")?;
            } else if r.starts_with('<') {
                children.push(Node::Element(self.element(depth + 1)?));
            } else {
                let end = r.find('<').unwrap_or(r.len());
                let t = decode(&r[..end])?;
                if !t.trim().is_empty() {
                    children.push(Node::Text(t));
                }
                self.pos += end;
            }
        }
    }
}

/// Parse a document and return its root element.
pub fn parse(src: &str) -> Result<Element, KshanaError> {
    let mut r = Reader {
        s: src.trim_start_matches('\u{feff}'),
        pos: 0,
    };
    r.misc()?;
    let root = r.element(0)?;
    r.misc()?;
    if !r.rest().trim().is_empty() {
        return Err(err("content after the root element"));
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_attributes_text_entities_and_cdata() {
        let doc = r#"<?xml version="1.0"?>
<!-- c --><a xmlns:x="u" k="1 &lt; 2"><x:b>t &amp; u</x:b><c/><![CDATA[<raw>]]></a>"#;
        let e = parse(doc).unwrap();
        assert_eq!(e.attr("k"), Some("1 < 2"));
        assert_eq!(e.child("b").unwrap().text(), "t & u");
        assert!(e.child("c").is_some());
        assert!(e.text().contains("<raw>"));
    }

    #[test]
    fn rejects_malformed_and_dtd_subsets() {
        assert!(parse("<a><b></a>").is_err());
        assert!(parse("<a k=v/>").is_err());
        assert!(parse("<!DOCTYPE a [<!ENTITY x \"y\">]><a/>").is_err());
        assert!(parse("<a>&bogus;</a>").is_err());
        assert!(parse("<a/><b/>").is_err());
    }

    #[test]
    fn escape_round_trips() {
        let s = "a<b & \"c\"\nd";
        let doc = format!("<a v=\"{}\"/>", escape(s));
        assert_eq!(parse(&doc).unwrap().attr("v"), Some(s));
    }
}
