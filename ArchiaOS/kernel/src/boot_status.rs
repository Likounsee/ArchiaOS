//! Pure, allocation-free classification of early boot log lines.
//! Keep this module host-testable so status coloring cannot silently regress.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    Failure,
    Success,
    Pending,
}

fn contains_ascii_case_insensitive(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window.eq_ignore_ascii_case(needle))
}

/// Classify a diagnostic conservatively: negative signals always win, and
/// unknown text remains pending instead of being presented as a success.
pub fn classify(line: &[u8]) -> Status {
    const FAILURES: &[&[u8]] = &[
        b"failed",
        b"failure",
        b"invalid",
        b"unavailable",
        b"unsupported",
        b"not initialized",
        b"not validated",
        b"not installed",
        b"not available",
        b"not supported",
        b"missing",
        b"cannot",
        b"unable to",
        b"could not",
        b"rejected",
        b"refusing",
        b"error",
    ];
    if FAILURES.iter().any(|needle| contains_ascii_case_insensitive(line, needle)) {
        return Status::Failure;
    }

    const SUCCESSES: &[&[u8]] = &[
        b"passed",
        b"validated",
        b"initialized",
        b"installed",
        b"complete",
        b"written",
        b"entered",
    ];
    if SUCCESSES.iter().any(|needle| contains_ascii_case_insensitive(line, needle)) {
        return Status::Success;
    }

    Status::Pending
}

#[cfg(test)]
mod tests {
    use super::{classify, Status};

    #[test]
    fn recognizes_success_case_insensitively() {
        assert_eq!(classify(b"IDT INITIALIZED"), Status::Success);
        assert_eq!(classify(b"BootInfo validated"), Status::Success);
    }

    #[test]
    fn negative_signals_override_positive_words() {
        assert_eq!(classify(b"Framebuffer not initialized"), Status::Failure);
        assert_eq!(classify(b"CPU feature unsupported after detection"), Status::Failure);
        assert_eq!(classify(b"ERROR: memory map validated"), Status::Failure);
        assert_eq!(classify(b"Could NOT allocate pages"), Status::Failure);
    }

    #[test]
    fn unknown_lines_remain_pending() {
        assert_eq!(classify(b"DETECTING CPU FEATURES"), Status::Pending);
        assert_eq!(classify(b""), Status::Pending);
    }
}
