//! Shared character, fallback and path policies for generated and supplied slugs.
use super::{AllowedCharacterSet, DotHandlingPolicy, EmptyOutputPolicy, SlugConfig, SlugError};
use std::collections::BTreeSet;
use unicode_general_category::{GeneralCategory, get_general_category};

/// Check the configured fallback before any input is read.
pub(super) fn validate_fallback(config: &SlugConfig) -> Result<(), SlugError> {
    let (fallback, generated) = match &config.empty_output_policy {
        EmptyOutputPolicy::KeepEmptySlug => return Ok(()),
        EmptyOutputPolicy::UseFallbackSlug(fallback) => (fallback, true),
        EmptyOutputPolicy::UseVerbatimFallbackSlug(fallback) => (fallback, false),
    };
    if fallback.is_empty() {
        return Err(SlugError::EmptyFallbackSlug);
    }
    if generated {
        validate_generated_grammar(fallback, config)?;
    }
    super::validate_slug(fallback, config.validation_policy)
}

/// Hold a fallback to the grammar the pipeline would have produced.
pub(super) fn validate_generated_grammar(
    value: &str,
    config: &SlugConfig,
) -> Result<(), SlugError> {
    if let Some(max_slug_chars) = config.max_slug_chars
        && value.chars().count() > max_slug_chars
    {
        return Err(SlugError::FallbackViolatesConfig {
            reason: "it is longer than max_slug_chars",
        });
    }
    if value.starts_with(config.replacement_delimiter)
        || value.ends_with(config.replacement_delimiter)
    {
        return Err(SlugError::FallbackViolatesConfig {
            reason: "a generated slug never begins or ends with the replacement delimiter",
        });
    }
    let mut previous: Option<char> = None;
    let mut previous_kept = false;
    let mut chars = value.chars().peekable();
    while let Some(scalar) = chars.next() {
        if scalar == config.replacement_delimiter && previous == Some(scalar) {
            return Err(SlugError::FallbackViolatesConfig {
                reason: "a generated slug never repeats the replacement delimiter",
            });
        }
        let kept = keeps(scalar, previous_kept, config.allowed_character_set);
        let ok = kept
            || scalar == config.replacement_delimiter
            || (scalar == '.'
                && should_preserve_dot(
                    previous,
                    chars.peek().copied(),
                    config.dot_handling_policy,
                ));
        if !ok {
            return Err(SlugError::FallbackViolatesConfig {
                reason: "it contains a character this configuration would have filtered out",
            });
        }
        if config.lowercase_enabled && scalar.to_lowercase().next() != Some(scalar) {
            return Err(SlugError::FallbackViolatesConfig {
                reason: "lowercasing is enabled and it is not lowercase",
            });
        }
        previous = Some(scalar);
        previous_kept = kept;
    }
    Ok(())
}

/// Refuse a delimiter this configuration would also keep from the input.
///
/// The delimiter plays three parts at once: a character a caller may type, the
/// marker for a run of filtered characters, and the sentinel trimmed off both
/// ends. Those only stay distinct while the delimiter is one the filter would
/// never keep. With `a` as the delimiter, `alpha beta` and `lpha beta` both
/// come out `lphabet` — the boundary is not inserted because the output already
/// ends in `a`, and the trim then eats real letters off real words. Two
/// different inputs, one slug, and characters the caller wrote silently gone.
///
/// So the invariant is checked before any input is read, rather than left to a
/// path validation that happens to catch some cases later.
pub(super) fn validate_replacement_delimiter(config: &SlugConfig) -> Result<(), SlugError> {
    let delimiter = config.replacement_delimiter;
    let reason = if keeps(delimiter, true, config.allowed_character_set) {
        Some("the allowed character set keeps it from the input")
    } else if delimiter == '.' && config.dot_handling_policy != DotHandlingPolicy::ReplaceAllDots {
        Some("the dot handling policy preserves it from the input")
    } else if config.lowercase_enabled && delimiter.to_lowercase().next() != Some(delimiter) {
        // Lowercasing now runs before the delimiter is inserted, so an
        // uppercase delimiter would be the one uppercase character in an
        // otherwise lowercased slug.
        Some("lowercasing changes it, so it would be the only uncased character in the slug")
    } else {
        None
    };
    match reason {
        Some(reason) => Err(SlugError::AmbiguousReplacementDelimiter { delimiter, reason }),
        None => Ok(()),
    }
}

pub(super) fn should_preserve_dot(
    previous: Option<char>,
    next: Option<char>,
    policy: DotHandlingPolicy,
) -> bool {
    match policy {
        DotHandlingPolicy::ReplaceAllDots => false,
        DotHandlingPolicy::PreserveAllDots => true,
        DotHandlingPolicy::PreserveDotsBetweenDecimalDigits => {
            matches!(
                (previous, next),
                (Some(previous), Some(next))
                    if is_unicode_decimal_digit(previous) && is_unicode_decimal_digit(next)
            )
        }
    }
}

/// Whether the character set keeps `ch`, given whether the character before it
/// was kept. Only a combining mark depends on that context: it is kept where it
/// completes a kept character and filtered where it would stand alone.
pub(super) fn keeps(
    ch: char,
    previous_kept: bool,
    allowed_character_set: AllowedCharacterSet,
) -> bool {
    match allowed_character_set {
        AllowedCharacterSet::UnicodeAlphanumericCharacters => ch.is_alphanumeric(),
        AllowedCharacterSet::AsciiAlphanumericCharacters => ch.is_ascii_alphanumeric(),
        AllowedCharacterSet::UnicodeLettersAndDecimalDigits => {
            is_unicode_letter(ch) || is_unicode_decimal_digit(ch)
        }
        AllowedCharacterSet::UnicodeLettersMarksAndDecimalDigits => {
            is_unicode_letter(ch)
                || is_unicode_decimal_digit(ch)
                || (previous_kept && is_combining_mark(ch))
        }
    }
}

pub(super) fn is_combining_mark(ch: char) -> bool {
    !ch.is_ascii()
        && matches!(
            get_general_category(ch),
            GeneralCategory::NonspacingMark
                | GeneralCategory::SpacingMark
                | GeneralCategory::EnclosingMark
        )
}

fn is_unicode_letter(ch: char) -> bool {
    if ch.is_ascii() {
        return ch.is_ascii_alphabetic();
    }
    matches!(
        get_general_category(ch),
        GeneralCategory::UppercaseLetter
            | GeneralCategory::LowercaseLetter
            | GeneralCategory::TitlecaseLetter
            | GeneralCategory::ModifierLetter
            | GeneralCategory::OtherLetter
    )
}

fn is_unicode_decimal_digit(ch: char) -> bool {
    if ch.is_ascii() {
        return ch.is_ascii_digit();
    }
    get_general_category(ch) == GeneralCategory::DecimalNumber
}

pub(super) fn validate_local_path_segment(value: &str) -> Result<(), SlugError> {
    if value.is_empty() {
        return Err(SlugError::EmptyPathSegment);
    }
    if value == "." || value == ".." {
        return Err(SlugError::PathSegmentDotValue);
    }

    for ch in value.chars() {
        match ch {
            '/' | '\\' => return Err(SlugError::PathSegmentSeparator { character: ch }),
            _ if ch.is_whitespace() => {
                return Err(SlugError::PathSegmentWhitespace { character: ch });
            }
            _ if ch.is_control() => {
                return Err(SlugError::PathSegmentControlCharacter { character: ch });
            }
            _ => {}
        }
    }

    Ok(())
}

pub(super) fn validate_url_path_segment(value: &str) -> Result<(), SlugError> {
    validate_local_path_segment(value)?;

    for ch in value.chars() {
        if matches!(ch, '?' | '#' | '%') {
            return Err(SlugError::UrlPathSegmentReservedCharacter { character: ch });
        }
    }

    Ok(())
}

/// Reject ambiguous maps before applying even an empty input.
pub(super) fn validate_transliteration_map(map: &[(&str, &str)]) -> Result<(), SlugError> {
    if map.iter().any(|(pattern, _)| pattern.is_empty()) {
        return Err(SlugError::EmptyTransliterationPattern);
    }
    let mut patterns = BTreeSet::new();
    if map.iter().any(|(pattern, _)| !patterns.insert(*pattern)) {
        return Err(SlugError::DuplicateTransliterationPattern);
    }

    Ok(())
}
