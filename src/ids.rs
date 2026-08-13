//! Process-local random identifiers.
//!
//! One generator, because two would eventually disagree about width or case and
//! a client comparing an ID from one resource against an ID from another would
//! be comparing different alphabets. Deliberately ungated: the `/api/v2` DTOs
//! and the orchestration-side execution registry both mint these, and the
//! registry exists in builds that have no web feature at all.

/// Generate a random 128-bit lowercase hexadecimal identifier.
///
/// Random rather than sequential on purpose: these name process-local episodes,
/// records, and correlations, and a guessable counter would let one caller
/// address another's binding by arithmetic.
pub fn new_hex_id() -> String {
    format!("{:032x}", rand::random::<u128>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_identifier_is_thirty_two_lowercase_hex_digits() {
        let id = new_hex_id();
        assert_eq!(id.len(), 32);
        assert!(
            id.bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "{id}"
        );
    }

    #[test]
    fn two_identifiers_do_not_collide() {
        assert_ne!(new_hex_id(), new_hex_id());
    }
}
