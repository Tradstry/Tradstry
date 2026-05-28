/// Metadata available to build a chunk's context header. All optionals are
/// omitted from the header when absent.
#[derive(Clone, Debug)]
pub struct DocMeta {
    pub source_type: String,
    pub title: String,
    pub date: Option<String>,
    pub symbol: Option<String>,
}

/// Build the deterministic context header prepended to a chunk before embedding.
/// Format: `[source_type · title · h1 › h2 · date · symbol]` with absent parts skipped.
pub fn deterministic_header(meta: &DocMeta, heading_path: &[String]) -> String {
    let mut parts: Vec<String> = vec![meta.source_type.clone(), meta.title.clone()];
    if !heading_path.is_empty() {
        parts.push(heading_path.join(" › "));
    }
    if let Some(d) = &meta.date {
        parts.push(d.clone());
    }
    if let Some(s) = &meta.symbol {
        parts.push(s.clone());
    }
    format!("[{}]", parts.join(" · "))
}

/// Compose the text that actually gets embedded + sparse-indexed: deterministic
/// header, then optional LLM situating blurb, then the raw chunk text. The raw
/// chunk is stored separately as `content` for display/citation.
pub fn compose_embedded_text(header: &str, llm_blurb: Option<&str>, raw: &str) -> String {
    match llm_blurb {
        Some(b) if !b.trim().is_empty() => format!("{header}\n{b}\n{raw}"),
        _ => format!("{header}\n{raw}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_includes_present_fields_only() {
        let meta = DocMeta {
            source_type: "journal_entry".into(),
            title: "Trade review for NVDA".into(),
            date: Some("2026-03-12".into()),
            symbol: Some("NVDA".into()),
        };
        let h = deterministic_header(&meta, &["Journal".into(), "March".into()]);
        assert!(h.contains("journal_entry"));
        assert!(h.contains("Trade review for NVDA"));
        assert!(h.contains("Journal › March"));
        assert!(h.contains("2026-03-12"));
        assert!(h.contains("NVDA"));
    }

    #[test]
    fn header_omits_absent_optionals() {
        let meta = DocMeta {
            source_type: "playbook".into(),
            title: "Breakout".into(),
            date: None,
            symbol: None,
        };
        let h = deterministic_header(&meta, &[]);
        assert!(h.contains("playbook"));
        assert!(h.contains("Breakout"));
        assert!(!h.contains("None"));
    }

    #[test]
    fn compose_orders_context_then_raw_and_excludes_nothing() {
        let out = compose_embedded_text("[hdr]", Some("This chunk covers exits."), "raw body");
        assert!(out.starts_with("[hdr]"));
        assert!(out.contains("This chunk covers exits."));
        assert!(out.trim_end().ends_with("raw body"));
    }

    #[test]
    fn compose_without_llm_blurb() {
        let out = compose_embedded_text("[hdr]", None, "raw body");
        assert_eq!(out, "[hdr]\nraw body");
    }
}
