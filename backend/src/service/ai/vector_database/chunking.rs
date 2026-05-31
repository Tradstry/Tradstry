use std::sync::LazyLock;

use tokenizers::Tokenizer;

use crate::service::ai::vector_database::blocks::{Block, BlockKind};

/// The exact tokenizer for the active embedding model
/// (`DEFAULT_VOYAGE_EMBEDDING_MODEL = "voyage-3.5"` in
/// `vector_database/client.rs`). Vendored from
/// `huggingface.co/voyageai/voyage-3.5/tokenizer.json`. If the embedding model
/// is ever changed, this asset must be re-vendored to match.
static VOYAGE_TOKENIZER_JSON: &[u8] = include_bytes!("assets/voyage-3.5-tokenizer.json");

/// Loaded once on first use. `expect` is safe: the asset is compiled into the
/// binary and validated by `tokenizer_loads_from_embedded_asset` (test below), so
/// this cannot fail at runtime. We deliberately do NOT fall back to a chars/4
/// estimate — a silent fallback would reintroduce the inaccuracy this removes.
static TOKENIZER: LazyLock<Tokenizer> = LazyLock::new(|| {
    Tokenizer::from_bytes(VOYAGE_TOKENIZER_JSON)
        .expect("embedded voyage-3.5 tokenizer.json must be valid")
});

// Tuning constants live in code (not env) by project convention.
pub const CHUNK_TARGET_TOKENS: usize = 450;
pub const CHUNK_MAX_TOKENS: usize = 512;
pub const CHUNK_OVERLAP_PCT: f32 = 0.15;
/// Docs whose total estimated tokens are <= this become a single chunk.
pub const MIN_CHUNK_TO_SPLIT_TOKENS: usize = 512;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub text: String,
    pub heading_path: Vec<String>,
    pub chunk_index: usize,
}

/// Exact voyage-3.5 token count (no special tokens added).
pub fn count_tokens(text: &str) -> usize {
    TOKENIZER
        .encode(text, false)
        .expect("voyage-3.5 tokenizer encode is infallible for valid UTF-8")
        .len()
}

/// Compute the enclosing heading path (H1 > H2 ...) at the position of `idx`
/// within `blocks`, by scanning backward and keeping the most recent heading at
/// each shallower-or-equal level.
fn heading_path_at(blocks: &[Block], idx: usize) -> Vec<String> {
    let mut stack: Vec<(u8, String)> = Vec::new();
    for block in blocks.iter().take(idx + 1) {
        if let BlockKind::Heading { level } = block.kind {
            while stack.last().map(|(l, _)| *l >= level).unwrap_or(false) {
                stack.pop();
            }
            stack.push((level, block.text.clone()));
        }
    }
    stack.into_iter().map(|(_, t)| t).collect()
}

/// Split a single oversized block's text into <= CHUNK_MAX_TOKENS pieces,
/// preferring sentence boundaries, falling back to words. Each unit is tokenized
/// once and summed (rather than re-encoding the growing candidate every step);
/// BPE counts are not perfectly additive across concatenation boundaries, but the
/// error is negligible at CHUNK_MAX_TOKENS scale.
fn split_oversized(text: &str) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut current = String::new();
    let mut current_tokens = 0usize;
    let units: Vec<&str> = text.split_inclusive(['.', '!', '?']).collect();
    let units = if units.len() <= 1 {
        text.split_whitespace().collect::<Vec<_>>()
    } else {
        units
    };
    for unit in units {
        let unit_tokens = count_tokens(unit);
        if current_tokens + unit_tokens > CHUNK_MAX_TOKENS && !current.is_empty() {
            pieces.push(current.trim().to_string());
            current = unit.to_string();
            current_tokens = unit_tokens;
        } else {
            if current.is_empty() {
                current = unit.to_string();
            } else {
                current.push(' ');
                current.push_str(unit);
            }
            current_tokens += unit_tokens;
        }
    }
    if !current.trim().is_empty() {
        pieces.push(current.trim().to_string());
    }
    pieces
}

/// Pack blocks into chunks at block boundaries up to CHUNK_TARGET_TOKENS, with
/// ~CHUNK_OVERLAP_PCT carry-over. Short docs collapse to one chunk. Oversized
/// single blocks are sentence/word-split.
pub fn chunk_blocks(blocks: &[Block]) -> Vec<Chunk> {
    if blocks.is_empty() {
        return Vec::new();
    }

    let total: usize = blocks.iter().map(|b| count_tokens(&b.text)).sum();
    if total <= MIN_CHUNK_TO_SPLIT_TOKENS {
        let text = blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        return vec![Chunk {
            text,
            heading_path: heading_path_at(blocks, blocks.len() - 1),
            chunk_index: 0,
        }];
    }

    let mut chunks: Vec<Chunk> = Vec::new();
    let mut buf: Vec<String> = Vec::new();
    let mut buf_tokens = 0usize;
    let flush = |chunks: &mut Vec<Chunk>, buf: &Vec<String>, at_idx: usize| {
        if buf.is_empty() {
            return;
        }
        chunks.push(Chunk {
            text: buf.join("\n"),
            heading_path: heading_path_at(blocks, at_idx),
            chunk_index: chunks.len(),
        });
    };

    for (i, block) in blocks.iter().enumerate() {
        let bt = count_tokens(&block.text);
        if bt > CHUNK_MAX_TOKENS {
            // Flush current buffer, then emit the oversized block as its own pieces.
            flush(&mut chunks, &buf, i.saturating_sub(1));
            buf.clear();
            buf_tokens = 0;
            for piece in split_oversized(&block.text) {
                chunks.push(Chunk {
                    text: piece,
                    heading_path: heading_path_at(blocks, i),
                    chunk_index: chunks.len(),
                });
            }
            continue;
        }
        if buf_tokens + bt > CHUNK_TARGET_TOKENS && !buf.is_empty() {
            flush(&mut chunks, &buf, i.saturating_sub(1));
            // Overlap: carry the trailing block into the next chunk.
            let carry = buf.last().cloned();
            buf.clear();
            buf_tokens = 0;
            if let Some(prev) = carry
                && (count_tokens(&prev) as f32)
                    <= CHUNK_TARGET_TOKENS as f32 * (CHUNK_OVERLAP_PCT + 0.5)
            {
                buf_tokens += count_tokens(&prev);
                buf.push(prev);
            }
        }
        buf.push(block.text.clone());
        buf_tokens += bt;
    }
    flush(&mut chunks, &buf, blocks.len() - 1);

    // Re-number chunk_index defensively (flush uses len() so already correct).
    for (i, c) in chunks.iter_mut().enumerate() {
        c.chunk_index = i;
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::ai::vector_database::blocks::{Block, BlockKind};

    fn para(text: &str) -> Block {
        Block {
            kind: BlockKind::Paragraph,
            text: text.into(),
        }
    }

    #[test]
    fn short_doc_is_single_chunk() {
        let blocks = vec![para("a short note about discipline")];
        let chunks = chunk_blocks(&blocks);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].chunk_index, 0);
        assert!(chunks[0].text.contains("discipline"));
    }

    #[test]
    fn long_doc_splits_into_multiple_chunks_with_overlap() {
        // Use ~30-word blocks (~37 tokens each) with unique markers so we can
        // verify the carry-overlap: the last block of chunk[i] must reappear as
        // the first block of chunk[i+1] at at least one boundary.
        // 20 blocks * ~37 tokens = ~740 tokens >> MIN_CHUNK_TO_SPLIT_TOKENS (512).
        // ~12 blocks fill CHUNK_TARGET_TOKENS (450), so at least two chunks form.
        // Each block's token estimate (~37) is well under the carry threshold
        // (450 * 0.65 ≈ 292), so overlap MUST fire.
        let num_blocks = 20usize;
        let blocks: Vec<Block> = (0..num_blocks)
            .map(|i| para(&format!("para{i} {}", "word ".repeat(29))))
            .collect();

        let chunks = chunk_blocks(&blocks);
        assert!(
            chunks.len() >= 2,
            "expected multiple chunks, got {}",
            chunks.len()
        );
        assert!(chunks.iter().all(|c| !c.text.is_empty()));
        for (i, c) in chunks.iter().enumerate() {
            assert_eq!(c.chunk_index, i);
        }

        // Verify that overlap actually fired at at least one chunk boundary.
        // The last paragraph in chunk[i] must equal the first paragraph in chunk[i+1].
        let mut overlap_found = false;
        for window in chunks.windows(2) {
            let last_of_prev = window[0].text.lines().last().unwrap_or("").to_string();
            let first_of_next = window[1].text.lines().next().unwrap_or("").to_string();
            if last_of_prev == first_of_next && !last_of_prev.is_empty() {
                overlap_found = true;
                break;
            }
        }
        assert!(
            overlap_found,
            "overlap never fired: chunk texts were {:?}",
            chunks
                .iter()
                .map(|c| &c.text[..c.text.len().min(60)])
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn heading_path_tracks_enclosing_headings() {
        let blocks = vec![
            Block {
                kind: BlockKind::Heading { level: 1 },
                text: "Journal".into(),
            },
            Block {
                kind: BlockKind::Heading { level: 2 },
                text: "March".into(),
            },
            para("traded the open"),
        ];
        let chunks = chunk_blocks(&blocks);
        assert_eq!(
            chunks[0].heading_path,
            vec!["Journal".to_string(), "March".to_string()]
        );
    }

    #[test]
    fn tokenizer_loads_from_embedded_asset() {
        // Guards the `.expect` in TOKENIZER / count_tokens: the embedded asset
        // must load and produce a positive token count for normal text.
        let n = count_tokens("hello world");
        assert!(n > 0, "expected a positive token count, got {n}");
    }

    #[test]
    fn count_tokens_differs_from_chars_over_four_on_dense_text() {
        // Number/punctuation-dense trade text tokenizes very differently from the
        // old chars/4 heuristic — this proves the real tokenizer is engaged.
        let s = "Bought 100 AAPL @ $192.34, stop $188.10, R:R 2.7";
        let chars_over_four = s.chars().count() / 4;
        let real = count_tokens(s);
        assert_ne!(
            real, chars_over_four,
            "count_tokens ({real}) should differ from chars/4 ({chars_over_four})"
        );
        assert!(real > 0);
    }

    #[test]
    fn oversized_block_splits_into_bounded_pieces() {
        // A single block far over CHUNK_MAX_TOKENS must split into multiple pieces,
        // each within the token cap.
        let big = "word ".repeat(4000); // ~4000 tokens, >> CHUNK_MAX_TOKENS (512)
        let blocks = vec![para(&big)];
        let chunks = chunk_blocks(&blocks);
        assert!(chunks.len() > 1, "expected a split, got {}", chunks.len());
        for c in &chunks {
            assert!(
                count_tokens(&c.text) <= CHUNK_MAX_TOKENS,
                "piece exceeded cap: {} tokens",
                count_tokens(&c.text)
            );
        }
    }
}
