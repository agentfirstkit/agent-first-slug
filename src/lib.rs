#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt;

use unicode_general_category::{GeneralCategory, get_general_category};

/// Rules used by [`slugify`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlugConfig {
    /// Character inserted for each run of filtered input characters.
    pub replacement_delimiter: char,
    /// Lowercase transliterated input before character filtering.
    pub lowercase_enabled: bool,
    /// Maximum number of Unicode scalar values to keep after lowercasing.
    pub max_slug_chars: Option<usize>,
    /// Character set kept after transliteration and optional lowercasing.
    pub allowed_character_set: AllowedCharacterSet,
    /// How dots are handled before other filtered characters become delimiters.
    pub dot_handling_policy: DotHandlingPolicy,
    /// Optional transliteration applied before lowercasing and character filtering.
    pub transliteration_policy: TransliterationPolicy,
    /// Optional validation applied after empty-output handling.
    pub validation_policy: SlugValidationPolicy,
    /// Behavior when the generated slug is empty.
    pub empty_output_policy: EmptyOutputPolicy,
}

impl Default for SlugConfig {
    fn default() -> Self {
        Self {
            replacement_delimiter: '-',
            lowercase_enabled: true,
            max_slug_chars: None,
            allowed_character_set: AllowedCharacterSet::UnicodeAlphanumericCharacters,
            dot_handling_policy: DotHandlingPolicy::ReplaceAllDots,
            transliteration_policy: TransliterationPolicy::None,
            validation_policy: SlugValidationPolicy::None,
            empty_output_policy: EmptyOutputPolicy::KeepEmptySlug,
        }
    }
}

/// Character sets that can pass through the slug filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllowedCharacterSet {
    /// Rust's Unicode alphanumeric predicate. It keeps the combining marks that
    /// carry the Unicode `Alphabetic` property — most dependent vowel signs —
    /// and filters the rest, so viramas and tone marks still split a word.
    UnicodeAlphanumericCharacters,
    /// ASCII letters and digits only.
    AsciiAlphanumericCharacters,
    /// Unicode letter categories plus Unicode decimal digits. Every combining
    /// mark becomes a delimiter, so scripts that write vowels, viramas, or tones
    /// as marks are split mid-word.
    UnicodeLettersAndDecimalDigits,
    /// [`Self::UnicodeLettersAndDecimalDigits`] plus combining marks (Mn, Mc,
    /// Me) that follow a kept character, so `ไม่ใช่`, `नमस्ते`, and `தமிழ்` stay
    /// whole. A mark with nothing kept before it is filtered like any other
    /// character, so a slug never begins with one or carries one after a
    /// delimiter or a dot.
    UnicodeLettersMarksAndDecimalDigits,
}

/// Dot handling before all other filtered characters become delimiters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DotHandlingPolicy {
    /// Treat every dot as a delimiter.
    ReplaceAllDots,
    /// Preserve every dot.
    PreserveAllDots,
    /// Preserve a dot only when the previous and next characters are decimal digits.
    PreserveDotsBetweenDecimalDigits,
}

/// Transliteration applied before lowercasing and character filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransliterationPolicy {
    /// Do not transliterate.
    None,
    /// Replace static string patterns with static replacement strings. At each
    /// position the longest matching pattern wins, so pattern order in the slice
    /// does not matter. Empty or repeated patterns are rejected before the input
    /// is processed, including repetitions with the same replacement.
    StaticReplacementMap(&'static [(&'static str, &'static str)]),
}

/// Optional validation applied after slug generation and empty-output handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlugValidationPolicy {
    /// Do not validate the resulting slug.
    None,
    /// Validate as one local filesystem path segment.
    LocalPathSegment,
    /// Validate as one URL path segment before percent-encoding.
    UrlPathSegment,
}

/// Behavior when the generated slug is empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmptyOutputPolicy {
    /// Return the empty slug.
    KeepEmptySlug,
    /// Replace the empty slug with a fallback held to the same rules.
    ///
    /// The fallback is checked against the configuration that produced the
    /// slug it stands in for — character set, delimiter, dot policy, case and
    /// `max_slug_chars` — not only against the target surface. Without that,
    /// an ASCII-only, 80-character configuration could still return an
    /// arbitrary-length mixed-case Unicode string and call it validated, and a
    /// caller reading "validated" as "matches my `SlugConfig`" would be wrong
    /// in exactly the case they reached for a fallback to avoid.
    UseFallbackSlug(String),
    /// Replace the empty slug with a fallback inserted exactly as written.
    ///
    /// Only the target surface is checked. This exists for a value that must
    /// match something already stored — a legacy identifier a caller cannot
    /// regenerate — and it means the result may not satisfy the configuration
    /// that produced it.
    UseVerbatimFallbackSlug(String),
}

/// Slug generation result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlugResult {
    /// Generated slug.
    pub slug: String,
    /// Whether the final slug differs from the input.
    pub changed_from_input: bool,
}

/// Errors returned by slug generation or validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlugError {
    /// A static transliteration map contains an empty pattern.
    EmptyTransliterationPattern,
    /// A static transliteration map repeats a pattern, even with the same replacement.
    DuplicateTransliterationPattern,
    /// A path segment cannot be empty.
    EmptyPathSegment,
    /// A path segment cannot contain `/` or `\`.
    PathSegmentSeparator { character: char },
    /// A path segment cannot contain Unicode whitespace.
    PathSegmentWhitespace { character: char },
    /// A path segment cannot contain control characters.
    PathSegmentControlCharacter { character: char },
    /// A path segment cannot be `.` or `..`.
    PathSegmentDotValue,
    /// A URL path segment cannot contain URL delimiter or raw percent characters.
    UrlPathSegmentReservedCharacter { character: char },
    /// A [`EmptyOutputPolicy::UseFallbackSlug`] value does not satisfy the
    /// configuration that produced the slug it replaces.
    ///
    /// Either choose a fallback the configuration could itself have produced,
    /// or say the value is exempt with
    /// [`EmptyOutputPolicy::UseVerbatimFallbackSlug`]. That remedy is named
    /// here rather than in the message, because the message is also read by
    /// callers who reach this crate through something other than its Rust API
    /// and have no such name to type.
    FallbackViolatesConfig {
        /// Which rule it broke.
        reason: &'static str,
    },
    /// The replacement delimiter is a character this configuration would also
    /// keep from the input, so the two could not be told apart.
    ///
    /// Choose one the filter removes, such as `-` or `_`. Like the fallback
    /// above, the remedy lives here and in each surface's own diagnostics
    /// rather than in the message every surface shares.
    AmbiguousReplacementDelimiter {
        /// The configured delimiter.
        delimiter: char,
        /// Which part of the configuration also claims it.
        reason: &'static str,
    },
}

impl fmt::Display for SlugError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTransliterationPattern => {
                write!(f, "transliteration patterns must not be empty")
            }
            Self::DuplicateTransliterationPattern => {
                write!(f, "transliteration patterns must be unique")
            }
            Self::EmptyPathSegment => write!(f, "path segment must not be empty"),
            Self::PathSegmentSeparator { character } => {
                write!(f, "path segment must not contain separator `{character}`")
            }
            Self::PathSegmentWhitespace { character } => {
                write!(f, "path segment must not contain whitespace `{character}`")
            }
            Self::FallbackViolatesConfig { reason } => write!(
                f,
                "fallback slug does not satisfy this configuration: {reason}"
            ),
            Self::AmbiguousReplacementDelimiter { delimiter, reason } => write!(
                f,
                "replacement delimiter `{delimiter}` is ambiguous: {reason}"
            ),
            Self::PathSegmentControlCharacter { character } => {
                write!(
                    f,
                    "path segment must not contain control character U+{:04X}",
                    *character as u32
                )
            }
            Self::PathSegmentDotValue => write!(f, "path segment must not be `.` or `..`"),
            Self::UrlPathSegmentReservedCharacter { character } => write!(
                f,
                "URL path segment must not contain reserved character `{character}`"
            ),
        }
    }
}

impl std::error::Error for SlugError {}

/// Generate a slug from `input` using explicit caller-provided rules.
///
/// Processing is deterministic:
///
/// 1. Reject a `replacement_delimiter` this configuration could not tell apart
///    from an input character.
/// 2. Reject empty or repeated transliteration patterns, then apply
///    [`TransliterationPolicy`]. Empty patterns take precedence over repetitions.
/// 3. Lowercase if `lowercase_enabled` is `true`.
/// 4. Walk characters left-to-right.
/// 5. Keep characters allowed by [`AllowedCharacterSet`].
/// 6. Apply [`DotHandlingPolicy`].
/// 7. Convert all other character runs to one `replacement_delimiter`.
/// 8. Trim leading and trailing `replacement_delimiter` characters.
/// 9. Apply `max_slug_chars` if present, never separating a kept character
///    from the combining marks after it, then strip any trailing
///    `replacement_delimiter` the cut exposed.
/// 10. Apply [`EmptyOutputPolicy`] if the slug is empty.
/// 11. Validate according to [`SlugValidationPolicy`].
///
/// Case mapping runs at step 3, before filtering, so every scalar in the result
/// is one the character set admits. It used to run after, and a case mapping
/// that expands — `İ` becomes `i` plus a combining dot — put characters in the
/// output that the character set would have rejected.
///
/// See the crate-level documentation (the README) for worked examples of each
/// target surface: default Unicode slugs, local path segments, URL path
/// segments, dot handling, and transliteration.
///
/// An [`EmptyOutputPolicy::UseFallbackSlug`] value is inserted as written
/// rather than run through the pipeline — steps 3 and 9 already ran on the
/// empty slug it replaces — but it is required to satisfy the same grammar
/// those steps would have produced.
/// [`EmptyOutputPolicy::UseVerbatimFallbackSlug`] waives that and checks only
/// the target surface.
pub fn slugify(input: &str, config: &SlugConfig) -> Result<SlugResult, SlugError> {
    validate_replacement_delimiter(config)?;
    let transliterated = apply_transliteration(input, config.transliteration_policy)?;
    // Case mapping runs before the filter, not after it.
    //
    // Unicode case mapping is not one scalar for one scalar: lowercasing
    // `U+0130 LATIN CAPITAL LETTER I WITH DOT ABOVE` yields `i` followed by a
    // combining dot. Lowercasing after the filter therefore put characters into
    // the slug that no character set here would have let through — so
    // `allowed_character_set` stopped describing the output alphabet, and a slug
    // could pass validation while breaking the configuration that generated it.
    // Filtering afterwards means whatever case mapping produces is judged by the
    // same rule as everything else.
    let cased = if config.lowercase_enabled {
        Cow::Owned(transliterated.to_lowercase())
    } else {
        transliterated
    };
    let filtered = filter_chars(&cased, config);
    let trimmed = filtered.trim_matches(config.replacement_delimiter);
    let truncated = match config.max_slug_chars {
        Some(max_slug_chars) => truncate_chars(
            trimmed.to_string(),
            max_slug_chars,
            config.replacement_delimiter,
            config.dot_handling_policy,
        ),
        None => trimmed.to_string(),
    };
    let slug = match (&config.empty_output_policy, truncated.is_empty()) {
        (EmptyOutputPolicy::UseFallbackSlug(fallback), true) => {
            validate_generated_grammar(fallback, config)?;
            fallback.clone()
        }
        (EmptyOutputPolicy::UseVerbatimFallbackSlug(fallback), true) => fallback.clone(),
        _ => truncated,
    };

    validate_slug(&slug, config.validation_policy)?;

    Ok(SlugResult {
        changed_from_input: slug != input,
        slug,
    })
}

/// Validate `value` according to a standalone validation policy.
pub fn validate_slug(value: &str, policy: SlugValidationPolicy) -> Result<(), SlugError> {
    match policy {
        SlugValidationPolicy::None => Ok(()),
        SlugValidationPolicy::LocalPathSegment => validate_local_path_segment(value),
        SlugValidationPolicy::UrlPathSegment => validate_url_path_segment(value),
    }
}

fn apply_transliteration(
    input: &str,
    policy: TransliterationPolicy,
) -> Result<Cow<'_, str>, SlugError> {
    let map = match policy {
        TransliterationPolicy::None => return Ok(Cow::Borrowed(input)),
        TransliterationPolicy::StaticReplacementMap(map) => map,
    };

    if map.iter().any(|(pattern, _)| pattern.is_empty()) {
        return Err(SlugError::EmptyTransliterationPattern);
    }
    let mut patterns = BTreeSet::new();
    if map.iter().any(|(pattern, _)| !patterns.insert(*pattern)) {
        return Err(SlugError::DuplicateTransliterationPattern);
    }

    let mut output = String::with_capacity(input.len());
    let mut remaining = input;
    while !remaining.is_empty() {
        if let Some((pattern, replacement)) = map
            .iter()
            .filter(|(pattern, _)| remaining.starts_with(*pattern))
            .max_by_key(|(pattern, _)| pattern.len())
        {
            output.push_str(replacement);
            remaining = &remaining[pattern.len()..];
            continue;
        }

        let Some(ch) = remaining.chars().next() else {
            break;
        };
        output.push(ch);
        remaining = &remaining[ch.len_utf8()..];
    }

    Ok(Cow::Owned(output))
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
/// Hold a fallback to the grammar the pipeline would have produced.
fn validate_generated_grammar(value: &str, config: &SlugConfig) -> Result<(), SlugError> {
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

fn validate_replacement_delimiter(config: &SlugConfig) -> Result<(), SlugError> {
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

fn filter_chars(input: &str, config: &SlugConfig) -> String {
    let mut output = String::with_capacity(input.len());
    let mut previous: Option<char> = None;
    let mut previous_kept = false;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        let kept = keeps(ch, previous_kept, config.allowed_character_set);
        if kept {
            output.push(ch);
        } else if ch == '.'
            && should_preserve_dot(previous, chars.peek().copied(), config.dot_handling_policy)
        {
            output.push('.');
        } else {
            push_replacement_delimiter(&mut output, config.replacement_delimiter);
        }
        previous = Some(ch);
        previous_kept = kept;
    }

    output
}

fn push_replacement_delimiter(output: &mut String, replacement_delimiter: char) {
    if output.is_empty() || output.ends_with(replacement_delimiter) {
        return;
    }
    output.push(replacement_delimiter);
}

/// Keep at most `max_chars` Unicode scalar values, then drop a trailing
/// `replacement_delimiter` the cut may have exposed. Filtering collapses interior
/// runs to one delimiter, so cutting mid-run can leave the slug ending in the
/// delimiter; trimming it keeps the result clean and never above `max_chars`.
/// A decimal-only dot exposed at the end is dropped because it lost its next digit.
///
/// A cut that lands on a combining mark backs up past the character it belongs
/// to, dropping that whole character rather than leaving its base without the
/// marks that complete it: `ไม่` cut to two scalars is `ไ`, not `ไม`, which is a
/// different word.
fn truncate_chars(
    mut value: String,
    max_chars: usize,
    replacement_delimiter: char,
    dots: DotHandlingPolicy,
) -> String {
    let boundaries: Vec<(usize, char)> = value.char_indices().collect();
    if max_chars < boundaries.len() {
        let mut cut = max_chars;
        while cut > 0
            && boundaries[cut].1 != replacement_delimiter
            && is_combining_mark(boundaries[cut].1)
        {
            cut -= 1;
        }
        value.truncate(boundaries[cut].0);
    }
    while value.ends_with(replacement_delimiter)
        || (dots == DotHandlingPolicy::PreserveDotsBetweenDecimalDigits && value.ends_with('.'))
    {
        value.pop();
    }
    value
}

fn should_preserve_dot(
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
fn keeps(ch: char, previous_kept: bool, allowed_character_set: AllowedCharacterSet) -> bool {
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

fn is_combining_mark(ch: char) -> bool {
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

fn validate_local_path_segment(value: &str) -> Result<(), SlugError> {
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

fn validate_url_path_segment(value: &str) -> Result<(), SlugError> {
    validate_local_path_segment(value)?;

    for ch in value.chars() {
        if matches!(ch, '?' | '#' | '%') {
            return Err(SlugError::UrlPathSegmentReservedCharacter { character: ch });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slug(input: &str, config: &SlugConfig) -> Result<String, SlugError> {
        slugify(input, config).map(|result| result.slug)
    }

    fn unicode_local_path_config() -> SlugConfig {
        SlugConfig {
            replacement_delimiter: '-',
            lowercase_enabled: true,
            max_slug_chars: None,
            allowed_character_set: AllowedCharacterSet::UnicodeAlphanumericCharacters,
            dot_handling_policy: DotHandlingPolicy::ReplaceAllDots,
            transliteration_policy: TransliterationPolicy::None,
            validation_policy: SlugValidationPolicy::LocalPathSegment,
            empty_output_policy: EmptyOutputPolicy::KeepEmptySlug,
        }
    }

    fn url_path_segment_config() -> SlugConfig {
        SlugConfig {
            replacement_delimiter: '-',
            lowercase_enabled: true,
            max_slug_chars: None,
            allowed_character_set: AllowedCharacterSet::UnicodeLettersAndDecimalDigits,
            dot_handling_policy: DotHandlingPolicy::PreserveDotsBetweenDecimalDigits,
            transliteration_policy: TransliterationPolicy::None,
            validation_policy: SlugValidationPolicy::UrlPathSegment,
            empty_output_policy: EmptyOutputPolicy::KeepEmptySlug,
        }
    }

    fn ascii_local_path_config_with_fallback() -> SlugConfig {
        SlugConfig {
            replacement_delimiter: '-',
            lowercase_enabled: true,
            max_slug_chars: None,
            allowed_character_set: AllowedCharacterSet::AsciiAlphanumericCharacters,
            dot_handling_policy: DotHandlingPolicy::ReplaceAllDots,
            transliteration_policy: TransliterationPolicy::None,
            validation_policy: SlugValidationPolicy::LocalPathSegment,
            empty_output_policy: EmptyOutputPolicy::UseFallbackSlug("fallback".to_string()),
        }
    }

    #[test]
    fn default_config_is_minimal_and_keeps_empty() {
        let config = SlugConfig::default();

        assert_eq!(
            slugify("", &config),
            Ok(SlugResult {
                slug: String::new(),
                changed_from_input: false,
            })
        );
        assert_eq!(slug("!!!", &config), Ok(String::new()));
    }

    #[test]
    fn unicode_local_path_segment_examples_pass_when_non_empty() {
        let config = unicode_local_path_config();

        assert_eq!(
            slug("現在的Nobody，未來的Somebody！", &config),
            Ok("現在的nobody-未來的somebody".to_string())
        );
        assert_eq!(
            slug("牙好，胃口就好，身体倍儿棒，吃嘛嘛香。", &config),
            Ok("牙好-胃口就好-身体倍儿棒-吃嘛嘛香".to_string())
        );
        assert_eq!(
            slug("お元気ですか？", &config),
            Ok("お元気ですか".to_string())
        );
        assert_eq!(
            slug("Ubuntu 16.04", &config),
            Ok("ubuntu-16-04".to_string())
        );
    }

    #[test]
    fn local_path_segment_validation_rejects_empty_slug_after_keep_empty() {
        assert_eq!(
            slug("!!!", &unicode_local_path_config()),
            Err(SlugError::EmptyPathSegment)
        );
    }

    #[test]
    fn url_path_segment_examples_pass() {
        let config = url_path_segment_config();

        assert_eq!(
            slug("Ubuntu 16.04", &config),
            Ok("ubuntu-16.04".to_string())
        );
        assert_eq!(
            slug("T.U.S.F.G.E.3.0.8", &config),
            Ok("t-u-s-f-g-e-3.0.8".to_string())
        );
        assert_eq!(
            slug(".18 increased ! ", &config),
            Ok("18-increased".to_string())
        );
        assert_eq!(
            slug("お元気ですか？", &config),
            Ok("お元気ですか".to_string())
        );
    }

    #[test]
    fn ascii_local_path_segment_examples_pass_with_configured_fallback() {
        let config = ascii_local_path_config_with_fallback();

        assert_eq!(slug("Hello 世界", &config), Ok("hello".to_string()));
        assert_eq!(
            slug("Ubuntu 16.04", &config),
            Ok("ubuntu-16-04".to_string())
        );
        assert_eq!(slug("你好，世界", &config), Ok("fallback".to_string()));
    }

    #[test]
    fn preserve_all_dots_keeps_every_dot() {
        let config = SlugConfig {
            dot_handling_policy: DotHandlingPolicy::PreserveAllDots,
            ..SlugConfig::default()
        };

        assert_eq!(slug("A.B..C", &config), Ok("a.b..c".to_string()));
    }

    #[test]
    fn ascii_character_set_removes_non_ascii_letters() {
        let config = SlugConfig {
            allowed_character_set: AllowedCharacterSet::AsciiAlphanumericCharacters,
            ..SlugConfig::default()
        };

        assert_eq!(slug("Cafe 世界 42", &config), Ok("cafe-42".to_string()));
    }

    #[test]
    fn unicode_letters_decimal_digits_excludes_letter_numbers() {
        let config = SlugConfig {
            allowed_character_set: AllowedCharacterSet::UnicodeLettersAndDecimalDigits,
            ..SlugConfig::default()
        };

        assert_eq!(slug("Chapter \u{2163}", &config), Ok("chapter".to_string()));
    }

    #[test]
    fn unicode_alphanumeric_keeps_letter_numbers() {
        let config = SlugConfig {
            allowed_character_set: AllowedCharacterSet::UnicodeAlphanumericCharacters,
            ..SlugConfig::default()
        };

        assert_eq!(
            slug("Chapter \u{2163}", &config),
            Ok("chapter-\u{2173}".to_string())
        );
    }

    #[test]
    fn static_transliteration_runs_before_filtering() {
        static MAP: &[(&str, &str)] = &[("Æ", "AE"), ("東京", "Tokyo")];
        let config = SlugConfig {
            allowed_character_set: AllowedCharacterSet::AsciiAlphanumericCharacters,
            transliteration_policy: TransliterationPolicy::StaticReplacementMap(MAP),
            ..SlugConfig::default()
        };

        assert_eq!(slug("Æther 東京", &config), Ok("aether-tokyo".to_string()));
    }

    #[test]
    fn transliteration_prefers_the_longest_match() {
        static MAP: &[(&str, &str)] = &[("a", "1"), ("ab", "8"), ("abc", "9")];
        static REVERSED: &[(&str, &str)] = &[("abc", "9"), ("ab", "8"), ("a", "1")];

        for map in [MAP, REVERSED] {
            let config = SlugConfig {
                transliteration_policy: TransliterationPolicy::StaticReplacementMap(map),
                ..SlugConfig::default()
            };
            // Unique overlapping patterns retain their longest-match result
            // regardless of slice order.
            for (input, expected) in [("abc", "9"), ("abx", "8x"), ("ax", "1x"), ("", "")] {
                assert_eq!(slug(input, &config), Ok(expected.to_string()));
            }
        }
    }

    #[test]
    fn duplicate_transliteration_patterns_are_rejected_before_input() {
        static SAME: &[(&str, &str)] = &[("ab", "x"), ("other", "z"), ("ab", "x")];
        static SAME_REORDERED: &[(&str, &str)] = &[("other", "z"), ("ab", "x"), ("ab", "x")];
        static CONFLICT: &[(&str, &str)] = &[("ab", "x"), ("ab", "y")];
        static CONFLICT_REVERSED: &[(&str, &str)] = &[("ab", "y"), ("ab", "x")];
        for map in [SAME, SAME_REORDERED, CONFLICT, CONFLICT_REVERSED] {
            let config = SlugConfig {
                transliteration_policy: TransliterationPolicy::StaticReplacementMap(map),
                ..SlugConfig::default()
            };
            for input in ["", "ordinary", "ab"] {
                assert_eq!(
                    slugify(input, &config),
                    Err(SlugError::DuplicateTransliterationPattern)
                );
            }
        }
        assert_eq!(
            SlugError::DuplicateTransliterationPattern.to_string(),
            "transliteration patterns must be unique"
        );
    }

    #[test]
    fn empty_transliteration_patterns_take_precedence_over_repetitions() {
        static EMPTY_FIRST: &[(&str, &str)] = &[("", "z"), ("ab", "x"), ("ab", "y")];
        static EMPTY_LAST: &[(&str, &str)] = &[("ab", "y"), ("ab", "x"), ("", "z")];
        static EMPTY_REPEATED: &[(&str, &str)] = &[("", "x"), ("", "y")];
        for map in [EMPTY_FIRST, EMPTY_LAST, EMPTY_REPEATED] {
            let config = SlugConfig {
                transliteration_policy: TransliterationPolicy::StaticReplacementMap(map),
                ..SlugConfig::default()
            };
            for input in ["", "ordinary", "ab"] {
                assert_eq!(
                    slugify(input, &config),
                    Err(SlugError::EmptyTransliterationPattern)
                );
            }
        }
    }

    #[test]
    fn empty_transliteration_pattern_is_rejected() {
        static MAP: &[(&str, &str)] = &[("", "x")];
        let config = SlugConfig {
            transliteration_policy: TransliterationPolicy::StaticReplacementMap(MAP),
            ..SlugConfig::default()
        };

        assert_eq!(
            slugify("anything", &config),
            Err(SlugError::EmptyTransliterationPattern)
        );
    }

    #[test]
    fn max_slug_chars_runs_after_lowercase_and_can_trigger_fallback() {
        let lower_before_max = SlugConfig {
            max_slug_chars: Some(1),
            ..SlugConfig::default()
        };
        assert_eq!(slug("\u{0130}", &lower_before_max), Ok("i".to_string()));

        // A zero-character budget cannot hold a fallback either: the budget is
        // the configuration's, and the fallback stands in for what it would
        // have produced.
        let fallback_after_max = SlugConfig {
            max_slug_chars: Some(0),
            empty_output_policy: EmptyOutputPolicy::UseFallbackSlug("fallback".to_string()),
            ..SlugConfig::default()
        };
        assert_eq!(
            slug("abc", &fallback_after_max),
            Err(SlugError::FallbackViolatesConfig {
                reason: "it is longer than max_slug_chars"
            })
        );
        let verbatim_after_max = SlugConfig {
            empty_output_policy: EmptyOutputPolicy::UseVerbatimFallbackSlug("fallback".to_string()),
            ..fallback_after_max
        };
        assert_eq!(slug("abc", &verbatim_after_max), Ok("fallback".to_string()));
    }

    #[test]
    fn every_scalar_in_a_slug_is_one_the_character_set_admits() {
        // `İ` lowercases to `i` plus a combining dot. Lowercasing after the
        // filter let that dot into the slug even though the character set would
        // not have kept it, so `allowed_character_set` stopped describing the
        // output. Now the mapping happens first and the dot is a filtered run
        // like any other.
        let config = SlugConfig::default();
        let slugged = slug("\u{0130}stanbul", &config).expect("a slug");
        assert_eq!(slugged, "i-stanbul");
        for scalar in slugged.chars() {
            assert!(
                keeps(scalar, true, config.allowed_character_set)
                    || scalar == config.replacement_delimiter,
                "U+{:04X} is in the slug but not in its alphabet",
                scalar as u32
            );
        }

        // ASCII-only sees the same expansion and keeps neither half of it.
        let ascii = SlugConfig {
            allowed_character_set: AllowedCharacterSet::AsciiAlphanumericCharacters,
            ..SlugConfig::default()
        };
        for scalar in slug("\u{0130}stanbul", &ascii).expect("a slug").chars() {
            assert!(scalar.is_ascii_alphanumeric() || scalar == '-');
        }
    }

    #[test]
    fn a_delimiter_the_filter_would_keep_is_refused_before_any_input_is_read() {
        // With `a` as the delimiter these two inputs both used to produce
        // `lphabet`: the run boundary was skipped because the output already
        // ended in `a`, and the trim then ate real letters. Different inputs,
        // one slug.
        let ambiguous = SlugConfig {
            replacement_delimiter: 'a',
            ..SlugConfig::default()
        };
        for input in ["alpha beta", "lpha beta"] {
            assert!(matches!(
                slugify(input, &ambiguous),
                Err(SlugError::AmbiguousReplacementDelimiter { delimiter: 'a', .. })
            ));
        }

        // A dot policy that preserves dots claims `.` the same way.
        assert!(matches!(
            slugify(
                "a.b c",
                &SlugConfig {
                    replacement_delimiter: '.',
                    dot_handling_policy: DotHandlingPolicy::PreserveAllDots,
                    ..SlugConfig::default()
                }
            ),
            Err(SlugError::AmbiguousReplacementDelimiter { delimiter: '.', .. })
        ));
        // ...and is fine when the policy replaces them.
        assert_eq!(
            slug(
                "a.b c",
                &SlugConfig {
                    replacement_delimiter: '.',
                    dot_handling_policy: DotHandlingPolicy::ReplaceAllDots,
                    ..SlugConfig::default()
                }
            ),
            Ok("a.b.c".to_string())
        );

        // A delimiter lowercasing would change cannot be the one uncased
        // character in a lowercased slug. Under ASCII-only, `Ä` gets past the
        // character-set rule and is caught by this one.
        assert_eq!(
            slugify(
                "a b",
                &SlugConfig {
                    replacement_delimiter: 'Ä',
                    allowed_character_set: AllowedCharacterSet::AsciiAlphanumericCharacters,
                    lowercase_enabled: true,
                    ..SlugConfig::default()
                }
            ),
            Err(SlugError::AmbiguousReplacementDelimiter {
                delimiter: 'Ä',
                reason: "lowercasing changes it, so it would be the only uncased character in the slug",
            })
        );
        // The same delimiter is fine when nothing is being lowercased.
        assert_eq!(
            slug(
                "a b",
                &SlugConfig {
                    replacement_delimiter: 'Ä',
                    allowed_character_set: AllowedCharacterSet::AsciiAlphanumericCharacters,
                    lowercase_enabled: false,
                    ..SlugConfig::default()
                }
            ),
            Ok("aÄb".to_string())
        );

        // The ordinary delimiters stay ordinary.
        for delimiter in ['-', '_', '~'] {
            assert!(
                slugify(
                    "hello world",
                    &SlugConfig {
                        replacement_delimiter: delimiter,
                        ..SlugConfig::default()
                    }
                )
                .is_ok(),
                "`{delimiter}` must remain usable"
            );
        }
    }

    #[test]
    fn decimal_dot_truncation_always_satisfies_fallback_grammar() {
        for input in [
            "Ubuntu 16.04",
            "Release 2024.10 notes",
            "版本 １２.３４ xyz",
        ] {
            for limit in 1..=input.chars().count() {
                let mut config = SlugConfig {
                    max_slug_chars: Some(limit),
                    dot_handling_policy: DotHandlingPolicy::PreserveDotsBetweenDecimalDigits,
                    ..SlugConfig::default()
                };
                let generated = slug(input, &config).unwrap();
                assert!(!generated.ends_with('.'), "{input} at {limit}");
                if !generated.is_empty() {
                    config.empty_output_policy =
                        EmptyOutputPolicy::UseFallbackSlug(generated.clone());
                    assert_eq!(slug("!!!", &config).unwrap(), generated);
                }
            }
        }
        for (input, limit, expected) in [
            ("Ubuntu 16.04", 10, "ubuntu-16"),
            ("Release 2024.10 notes", 13, "release-2024"),
        ] {
            let config = SlugConfig {
                max_slug_chars: Some(limit),
                dot_handling_policy: DotHandlingPolicy::PreserveDotsBetweenDecimalDigits,
                ..SlugConfig::default()
            };
            assert_eq!(slug(input, &config).unwrap(), expected);
        }
        let config = SlugConfig {
            max_slug_chars: Some(2),
            dot_handling_policy: DotHandlingPolicy::PreserveAllDots,
            ..SlugConfig::default()
        };
        assert_eq!(slug("a.b", &config).unwrap(), "a.");
    }

    #[test]
    fn generated_fallback_rejects_repeated_delimiters_but_verbatim_keeps_them() {
        let mut config = SlugConfig {
            empty_output_policy: EmptyOutputPolicy::UseFallbackSlug("a--b".into()),
            ..SlugConfig::default()
        };
        assert_eq!(
            slug("!!!", &config),
            Err(SlugError::FallbackViolatesConfig {
                reason: "a generated slug never repeats the replacement delimiter",
            })
        );
        config.empty_output_policy = EmptyOutputPolicy::UseVerbatimFallbackSlug("a--b".into());
        assert_eq!(slug("!!!", &config).unwrap(), "a--b");
        config.empty_output_policy = EmptyOutputPolicy::UseFallbackSlug("a-b".into());
        assert_eq!(slug("!!!", &config).unwrap(), "a-b");
    }

    #[test]
    fn truncation_strips_trailing_delimiter_the_cut_exposes() {
        let config = SlugConfig {
            max_slug_chars: Some(6),
            ..SlugConfig::default()
        };
        // "hello-world" cut to 6 chars is "hello-"; the exposed delimiter is dropped.
        assert_eq!(slug("hello world", &config), Ok("hello".to_string()));

        // A cut that lands mid-word keeps the partial word unchanged.
        let mid_word = SlugConfig {
            max_slug_chars: Some(4),
            ..SlugConfig::default()
        };
        assert_eq!(slug("hello world", &mid_word), Ok("hell".to_string()));

        // A cut landing right after a delimiter drops it, leaving the leading token.
        let after_delimiter = SlugConfig {
            max_slug_chars: Some(2),
            ..SlugConfig::default()
        };
        assert_eq!(slug("a bb cc", &after_delimiter), Ok("a".to_string()));
    }

    #[test]
    fn validation_runs_after_fallback() {
        let valid_fallback = ascii_local_path_config_with_fallback();
        assert_eq!(slug("你好", &valid_fallback), Ok("fallback".to_string()));

        // A verbatim fallback is judged only by the target surface, so this is
        // where surface validation is the thing that catches it.
        let invalid_fallback = SlugConfig {
            empty_output_policy: EmptyOutputPolicy::UseVerbatimFallbackSlug(
                "bad/fallback".to_string(),
            ),
            validation_policy: SlugValidationPolicy::LocalPathSegment,
            ..SlugConfig::default()
        };
        assert_eq!(
            slug("!!!", &invalid_fallback),
            Err(SlugError::PathSegmentSeparator { character: '/' })
        );
    }

    #[test]
    fn a_fallback_must_satisfy_the_configuration_it_stands_in_for() {
        // The case the report names: an ASCII-only, length-capped configuration
        // used to accept any fallback at all and still report it as validated.
        let ascii_capped = |fallback: &str| SlugConfig {
            allowed_character_set: AllowedCharacterSet::AsciiAlphanumericCharacters,
            max_slug_chars: Some(8),
            empty_output_policy: EmptyOutputPolicy::UseFallbackSlug(fallback.to_string()),
            ..SlugConfig::default()
        };
        for (fallback, expected) in [
            (
                "Ünïcode",
                "it contains a character this configuration would have filtered out",
            ),
            ("MixedCase", "it is longer than max_slug_chars"),
            ("waytoolongfallback", "it is longer than max_slug_chars"),
            (
                "-leading",
                "a generated slug never begins or ends with the replacement delimiter",
            ),
        ] {
            assert_eq!(
                slug("!!!", &ascii_capped(fallback)),
                Err(SlugError::FallbackViolatesConfig { reason: expected }),
                "fallback {fallback:?}"
            );
        }
        // One the configuration could itself have produced is fine.
        assert_eq!(
            slug("!!!", &ascii_capped("untitled")),
            Ok("untitled".to_string())
        );

        // And a caller who must match something already stored says so.
        assert_eq!(
            slug(
                "!!!",
                &SlugConfig {
                    allowed_character_set: AllowedCharacterSet::AsciiAlphanumericCharacters,
                    max_slug_chars: Some(8),
                    empty_output_policy: EmptyOutputPolicy::UseVerbatimFallbackSlug(
                        "Legacy Ünïcode Name".to_string()
                    ),
                    ..SlugConfig::default()
                }
            ),
            Ok("Legacy Ünïcode Name".to_string())
        );
    }

    #[test]
    fn no_validation_accepts_raw_unsafe_value() {
        assert_eq!(validate_slug("", SlugValidationPolicy::None), Ok(()));
        assert_eq!(validate_slug("../x", SlugValidationPolicy::None), Ok(()));
    }

    #[test]
    fn local_path_segment_validation_rejects_unsafe_values() {
        assert_eq!(
            validate_slug("", SlugValidationPolicy::LocalPathSegment),
            Err(SlugError::EmptyPathSegment)
        );
        assert_eq!(
            validate_slug("a/b", SlugValidationPolicy::LocalPathSegment),
            Err(SlugError::PathSegmentSeparator { character: '/' })
        );
        assert_eq!(
            validate_slug("a\\b", SlugValidationPolicy::LocalPathSegment),
            Err(SlugError::PathSegmentSeparator { character: '\\' })
        );
        assert_eq!(
            validate_slug("a b", SlugValidationPolicy::LocalPathSegment),
            Err(SlugError::PathSegmentWhitespace { character: ' ' })
        );
        assert_eq!(
            validate_slug("a\u{0007}b", SlugValidationPolicy::LocalPathSegment),
            Err(SlugError::PathSegmentControlCharacter {
                character: '\u{0007}'
            })
        );
        assert_eq!(
            validate_slug(".", SlugValidationPolicy::LocalPathSegment),
            Err(SlugError::PathSegmentDotValue)
        );
        assert_eq!(
            validate_slug("..", SlugValidationPolicy::LocalPathSegment),
            Err(SlugError::PathSegmentDotValue)
        );
    }

    #[test]
    fn url_path_segment_validation_rejects_url_delimiters_and_raw_percent() {
        assert_eq!(
            validate_slug("a?b", SlugValidationPolicy::UrlPathSegment),
            Err(SlugError::UrlPathSegmentReservedCharacter { character: '?' })
        );
        assert_eq!(
            validate_slug("a#b", SlugValidationPolicy::UrlPathSegment),
            Err(SlugError::UrlPathSegmentReservedCharacter { character: '#' })
        );
        assert_eq!(
            validate_slug("a%b", SlugValidationPolicy::UrlPathSegment),
            Err(SlugError::UrlPathSegmentReservedCharacter { character: '%' })
        );
        assert_eq!(
            validate_slug("a/b", SlugValidationPolicy::UrlPathSegment),
            Err(SlugError::PathSegmentSeparator { character: '/' })
        );
        assert_eq!(
            validate_slug("safe-現在-16.04", SlugValidationPolicy::UrlPathSegment),
            Ok(())
        );
    }

    fn marks_config() -> SlugConfig {
        SlugConfig {
            allowed_character_set: AllowedCharacterSet::UnicodeLettersMarksAndDecimalDigits,
            ..SlugConfig::default()
        }
    }

    #[test]
    fn only_the_marks_set_keeps_words_written_with_combining_marks_whole() {
        let alphanumeric = SlugConfig::default();
        let letters_digits = SlugConfig {
            allowed_character_set: AllowedCharacterSet::UnicodeLettersAndDecimalDigits,
            ..SlugConfig::default()
        };
        let marks = marks_config();
        // Thai and Lao tone marks, a Devanagari virama, a Tamil pulli: none of
        // them carries the `Alphabetic` property, so only the marks set keeps them.
        for (input, alphanumeric_slug, letters_digits_slug) in [
            ("ไม่ใช่", "ไม-ใช", "ไม-ใช"),
            ("ກ່ອນ", "ກ-ອນ", "ກ-ອນ"),
            ("नमस्ते", "नमस-ते", "नमस-त"),
            ("தமிழ்", "தமிழ", "தம-ழ"),
        ] {
            assert_eq!(
                slug(input, &alphanumeric),
                Ok(alphanumeric_slug.to_string())
            );
            assert_eq!(
                slug(input, &letters_digits),
                Ok(letters_digits_slug.to_string())
            );
            assert_eq!(slug(input, &marks), Ok(input.to_string()));
        }
        assert_eq!(slug("नमस्ते दुनिया", &marks), Ok("नमस्ते-दुनिया".to_string()));
        // A decomposed accent completes its letter instead of splitting it.
        assert_eq!(
            slug("Cafe\u{301} Noir", &marks),
            Ok("cafe\u{301}-noir".to_string())
        );
        // The set admits the combining dot `İ` lowercases to, so it stays.
        assert_eq!(
            slug("\u{0130}stanbul", &marks),
            Ok("i\u{307}stanbul".to_string())
        );
    }

    #[test]
    fn a_mark_with_nothing_kept_before_it_is_filtered() {
        let marks = marks_config();
        assert_eq!(slug("\u{301}abc", &marks), Ok("abc".to_string()));
        assert_eq!(slug("a \u{301}b", &marks), Ok("a-b".to_string()));
        assert_eq!(slug("\u{E48}\u{E48}", &marks), Ok(String::new()));
        let dots = SlugConfig {
            dot_handling_policy: DotHandlingPolicy::PreserveAllDots,
            ..marks_config()
        };
        assert_eq!(slug("1.\u{301}2", &dots), Ok("1.-2".to_string()));
    }

    #[test]
    fn truncation_never_leaves_a_character_without_its_marks() {
        let cut = |max| SlugConfig {
            max_slug_chars: Some(max),
            ..marks_config()
        };
        // ไ ม ่ ใ ช ่: a cut after `ม` would drop its tone mark and spell a
        // different word, so the whole character goes.
        assert_eq!(slug("ไม่ใช่", &cut(2)), Ok("ไ".to_string()));
        assert_eq!(slug("ไม่ใช่", &cut(3)), Ok("ไม่".to_string()));
        assert_eq!(slug("ไม่ใช่", &cut(1)), Ok("ไ".to_string()));
        assert_eq!(slug("\u{E44}\u{E21}\u{E48}", &cut(0)), Ok(String::new()));

        // The default set keeps Devanagari vowel signs, so it gets the same
        // protection: न म स - त े cut to five is not `नमस-त`.
        let default_cut = SlugConfig {
            max_slug_chars: Some(5),
            ..SlugConfig::default()
        };
        assert_eq!(slug("नमस्ते", &default_cut), Ok("नमस".to_string()));
    }

    #[test]
    fn the_marks_set_refuses_a_mark_as_its_delimiter() {
        let config = SlugConfig {
            replacement_delimiter: '\u{301}',
            ..marks_config()
        };
        assert_eq!(
            slugify("a b", &config),
            Err(SlugError::AmbiguousReplacementDelimiter {
                delimiter: '\u{301}',
                reason: "the allowed character set keeps it from the input",
            })
        );
    }

    #[test]
    fn a_fallback_under_the_marks_set_follows_the_same_attachment_rule() {
        let fallback = |value: &str| SlugConfig {
            empty_output_policy: EmptyOutputPolicy::UseFallbackSlug(value.to_string()),
            ..marks_config()
        };
        assert_eq!(slug("!!!", &fallback("ไม่-ใช่")), Ok("ไม่-ใช่".to_string()));
        for orphaned in ["\u{301}a", "a-\u{301}b"] {
            assert_eq!(
                slug("!!!", &fallback(orphaned)),
                Err(SlugError::FallbackViolatesConfig {
                    reason: "it contains a character this configuration would have filtered out",
                }),
                "{orphaned:?}"
            );
        }
    }
}
