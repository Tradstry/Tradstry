const INTERNAL_ID_REPLACEMENT: &str = "the tagged trade";
const UUID_LENGTH: usize = 36;

fn is_hex(character: char) -> bool {
    character.is_ascii_hexdigit()
}

fn is_uuid_prefix(candidate: &str) -> bool {
    if candidate.len() > UUID_LENGTH || !candidate.is_ascii() {
        return false;
    }

    candidate.chars().enumerate().all(|(index, character)| {
        if matches!(index, 8 | 13 | 18 | 23) {
            character == '-'
        } else {
            is_hex(character)
        }
    })
}

/// Redacts UUID-shaped internal identifiers without breaking streaming when an
/// identifier is split across multiple Gemini chunks.
#[derive(Default)]
pub struct InternalIdRedactor {
    pending: String,
}

impl InternalIdRedactor {
    pub fn push(&mut self, chunk: &str) -> String {
        let mut visible = String::new();

        for character in chunk.chars() {
            self.pending.push(character);

            loop {
                if is_uuid_prefix(&self.pending) {
                    if self.pending.len() == UUID_LENGTH {
                        visible.push_str(INTERNAL_ID_REPLACEMENT);
                        self.pending.clear();
                    }
                    break;
                }

                let first = self.pending.remove(0);
                visible.push(first);
                if self.pending.is_empty() {
                    break;
                }
            }
        }

        visible
    }

    pub fn finish(&mut self) -> String {
        std::mem::take(&mut self.pending)
    }
}

pub fn redact_internal_ids(text: &str) -> String {
    let mut redactor = InternalIdRedactor::default();
    let mut visible = redactor.push(text);
    visible.push_str(&redactor.finish());
    visible
}

#[cfg(test)]
mod tests {
    use super::{InternalIdRedactor, redact_internal_ids};

    const TRADE_ID: &str = "93a79073-f4ef-4cd1-8ec9-5a14b27a2593";

    #[test]
    fn redacts_internal_uuid_from_complete_text() {
        assert_eq!(
            redact_internal_ids(&format!("Trade ID {TRADE_ID} is profitable")),
            "Trade ID the tagged trade is profitable"
        );
    }

    #[test]
    fn redacts_identifier_split_across_stream_chunks() {
        let mut redactor = InternalIdRedactor::default();
        let mut visible = String::new();
        visible.push_str(&redactor.push("Review 93a79073-f4ef-"));
        visible.push_str(&redactor.push("4cd1-8ec9-5a14b27a2593 now"));
        visible.push_str(&redactor.finish());

        assert_eq!(visible, "Review the tagged trade now");
    }

    #[test]
    fn leaves_dates_symbols_and_partial_ids_unchanged() {
        assert_eq!(
            redact_internal_ids("SMCI on 2026-08-06, reference 93a79073"),
            "SMCI on 2026-08-06, reference 93a79073"
        );
    }
}
