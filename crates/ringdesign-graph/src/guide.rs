//! Plain-language help for every node kind, value type and graph concept, read from `guide.md`.

use std::collections::BTreeMap;
use std::sync::OnceLock;

const GUIDE: &str = include_str!("guide.md");

/// One section of the guide: its first paragraph and everything after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub summary: &'static str,
    pub body: &'static str,
}

fn sections() -> &'static BTreeMap<&'static str, Entry> {
    static SECTIONS: OnceLock<BTreeMap<&'static str, Entry>> = OnceLock::new();
    SECTIONS.get_or_init(|| parse(GUIDE))
}

/// The `## key` sections of a guide, each split into its first paragraph and the rest.
pub fn parse(text: &'static str) -> BTreeMap<&'static str, Entry> {
    let mut heads: Vec<(usize, usize, &'static str)> = Vec::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if let Some(key) = line.strip_prefix("## ") {
            heads.push((offset, offset + line.len(), key.trim()));
        }
        offset += line.len();
    }
    let mut out = BTreeMap::new();
    for (i, (_, start, key)) in heads.iter().enumerate() {
        let end = heads.get(i + 1).map_or(text.len(), |h| h.0);
        let section = text[*start..end].trim();
        let (summary, body) = section.split_once("\n\n").map_or((section, ""), |(s, b)| (s.trim(), b.trim()));
        out.insert(*key, Entry { summary, body });
    }
    out
}

impl Entry {
    /// The summary as one line.
    pub fn line(&self) -> String {
        self.summary.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

/// The help for a node kind, by registry key.
pub fn node(key: &str) -> Option<Entry> {
    sections().get(key).copied()
}

/// The help for a value type, by its label (`number`, `window`, `alpha source`).
pub fn kind(label: &str) -> Option<Entry> {
    sections().get(format!("type:{label}").as_str()).copied()
}

/// The help on a concept: `overview`, `layers`, `wiring`.
pub fn concept(name: &str) -> Option<Entry> {
    sections().get(format!("concept:{name}").as_str()).copied()
}

/// Every node key the guide covers.
pub fn node_keys() -> impl Iterator<Item = &'static str> {
    sections().keys().copied().filter(|k| !k.contains(':'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_split_into_summary_and_body() {
        let text = "# Guide\n<!-- ignored -->\n\n## window\nLimits a layer to an arc.\n\nMore here.\n- a bullet\n\n## type:window\nAn arc round the ring.\n";
        let s = parse(text);
        assert_eq!(s["window"], Entry { summary: "Limits a layer to an arc.", body: "More here.\n- a bullet" });
        assert_eq!(s["type:window"].summary, "An arc round the ring.");
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn every_node_and_type_has_its_help() {
        let reg = crate::registry::Registry::builtin();
        let missing: Vec<&str> = reg.keys().filter(|k| node(k).is_none()).collect();
        assert!(missing.is_empty(), "no guide section for {missing:?}");
        for k in crate::value::ValueKind::ALL {
            assert!(kind(k.label()).is_some(), "no guide section for the {} type", k.label());
        }
        for c in ["overview", "layers", "wiring"] {
            assert!(concept(c).is_some(), "no {c} concept");
        }
        for key in node_keys() {
            let e = node(key).unwrap();
            assert!(!e.summary.is_empty() && e.line().chars().count() <= 200, "{key}: the summary is a short first paragraph");
        }
    }
}
