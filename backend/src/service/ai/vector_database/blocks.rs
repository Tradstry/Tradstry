use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockKind {
    Heading { level: u8 },
    Paragraph,
    ListItem,
    Quote,
    Code,
    Field { name: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub kind: BlockKind,
    pub text: String,
}

impl Block {
    pub fn field(name: impl Into<String>, text: impl Into<String>) -> Self {
        Block {
            kind: BlockKind::Field { name: name.into() },
            text: text.into(),
        }
    }
}

/// Concatenate all descendant `text` nodes of a Lexical node into one string.
fn collect_text(node: &Value) -> String {
    let mut out = String::new();
    fn walk(node: &Value, out: &mut String) {
        if let Some(t) = node.get("text").and_then(Value::as_str) {
            out.push_str(t);
        }
        if let Some(children) = node.get("children").and_then(Value::as_array) {
            for child in children {
                walk(child, out);
            }
        }
    }
    walk(node, &mut out);
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Walk a Lexical `documentJson` tree into block-level `Block`s, preserving
/// heading levels and block boundaries. Unknown/inline node types are ignored
/// at the top level; `list` nodes expand into one `ListItem` block per item.
pub fn extract_notebook_blocks(document_json: &str) -> Vec<Block> {
    let parsed: Value = match serde_json::from_str(document_json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let root_children = parsed
        .get("root")
        .and_then(|r| r.get("children"))
        .and_then(Value::as_array);
    let Some(top) = root_children else {
        return Vec::new();
    };

    let mut blocks = Vec::new();
    for node in top {
        let node_type = node.get("type").and_then(Value::as_str).unwrap_or("");
        match node_type {
            "heading" => {
                let level = node
                    .get("tag")
                    .and_then(Value::as_str)
                    .and_then(|t| t.strip_prefix('h'))
                    .and_then(|n| n.parse::<u8>().ok())
                    .unwrap_or(1);
                push_if_nonempty(
                    &mut blocks,
                    BlockKind::Heading { level },
                    collect_text(node),
                );
            }
            "paragraph" => push_if_nonempty(&mut blocks, BlockKind::Paragraph, collect_text(node)),
            "quote" => push_if_nonempty(&mut blocks, BlockKind::Quote, collect_text(node)),
            "code" => push_if_nonempty(&mut blocks, BlockKind::Code, collect_text(node)),
            "list" => {
                if let Some(items) = node.get("children").and_then(Value::as_array) {
                    for item in items {
                        push_if_nonempty(&mut blocks, BlockKind::ListItem, collect_text(item));
                    }
                }
            }
            _ => {
                // Fallback: capture any other block carrying text as a paragraph.
                let text = collect_text(node);
                push_if_nonempty(&mut blocks, BlockKind::Paragraph, text);
            }
        }
    }
    blocks
}

fn push_if_nonempty(blocks: &mut Vec<Block>, kind: BlockKind, text: String) {
    if !text.trim().is_empty() {
        blocks.push(Block { kind, text });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraph_and_heading_extracted_with_levels() {
        let doc = r#"{"root":{"children":[
            {"type":"heading","tag":"h2","children":[{"type":"text","text":"Setup"}]},
            {"type":"paragraph","children":[{"type":"text","text":"Bought "},{"type":"text","text":"NVDA"}]}
        ]}}"#;
        let blocks = extract_notebook_blocks(doc);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].kind, BlockKind::Heading { level: 2 });
        assert_eq!(blocks[0].text, "Setup");
        assert_eq!(blocks[1].kind, BlockKind::Paragraph);
        assert_eq!(blocks[1].text, "Bought NVDA");
    }

    #[test]
    fn list_items_and_quote_and_code() {
        let doc = r#"{"root":{"children":[
            {"type":"list","children":[
                {"type":"listitem","children":[{"type":"text","text":"rule one"}]},
                {"type":"listitem","children":[{"type":"text","text":"rule two"}]}
            ]},
            {"type":"quote","children":[{"type":"text","text":"stay patient"}]},
            {"type":"code","children":[{"type":"text","text":"R = 2"}]}
        ]}}"#;
        let blocks = extract_notebook_blocks(doc);
        assert_eq!(
            blocks
                .iter()
                .filter(|b| b.kind == BlockKind::ListItem)
                .count(),
            2
        );
        assert!(
            blocks
                .iter()
                .any(|b| b.kind == BlockKind::Quote && b.text == "stay patient")
        );
        assert!(
            blocks
                .iter()
                .any(|b| b.kind == BlockKind::Code && b.text == "R = 2")
        );
    }

    #[test]
    fn empty_blocks_skipped_and_bad_json_is_empty() {
        let doc = r#"{"root":{"children":[{"type":"paragraph","children":[]}]}}"#;
        assert!(extract_notebook_blocks(doc).is_empty());
        assert!(extract_notebook_blocks("not json").is_empty());
    }
}
