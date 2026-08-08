pub mod comparison;
pub mod report;
pub mod research;

pub(super) fn truncate_utf8(value: &str, max_bytes: usize) -> &str {
    if value.len() <= max_bytes {
        value
    } else {
        &value[..value.floor_char_boundary(max_bytes)]
    }
}

#[cfg(test)]
mod tests {
    use super::truncate_utf8;

    #[test]
    fn truncates_without_splitting_unicode() {
        assert_eq!(truncate_utf8("abc🙂def", 6), "abc");
        assert_eq!(truncate_utf8("abc", 6), "abc");
    }
}
