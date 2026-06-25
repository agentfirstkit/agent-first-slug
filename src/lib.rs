#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]

use std::borrow::Cow;
use std::fmt;

use unicode_general_category::{get_general_category, GeneralCategory};

/// Rules used by [`slugify`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlugConfig {
    /// Character inserted for each run of filtered input characters.
    pub replacement_delimiter: char,
    /// Lowercase the generated slug after delimiter trimming.
    pub lowercase_enabled: bool,
    /// Maximum number of Unicode scalar values to keep after lowercasing.
    pub max_slug_chars: Option<usize>,
    /// Character set kept from the input after transliteration.
    pub allowed_character_set: AllowedCharacterSet,
    /// How dots are handled before other filtered characters become delimiters.
    pub dot_handling_policy: DotHandlingPolicy,
    /// Optional transliteration applied before character filtering.
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
    /// Rust's Unicode alphanumeric predicate.
    UnicodeAlphanumericCharacters,
    /// ASCII letters and digits only.
    AsciiAlphanumericCharacters,
    /// Unicode letter categories plus Unicode decimal digits.
    UnicodeLettersAndDecimalDigits,
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

/// Transliteration applied before character filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransliterationPolicy {
    /// Do not transliterate.
    None,
    /// Replace static string patterns with static replacement strings. At each
    /// position the longest matching pattern wins, so pattern order in the slice
    /// does not matter.
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
    /// Replace the empty slug with a caller-provided fallback.
    UseFallbackSlug(String),
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
}

impl fmt::Display for SlugError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTransliterationPattern => {
                write!(f, "transliteration patterns must not be empty")
            }
            Self::EmptyPathSegment => write!(f, "path segment must not be empty"),
            Self::PathSegmentSeparator { character } => {
                write!(f, "path segment must not contain separator `{character}`")
            }
            Self::PathSegmentWhitespace { character } => {
                write!(f, "path segment must not contain whitespace `{character}`")
            }
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
/// 1. Apply [`TransliterationPolicy`].
/// 2. Walk characters left-to-right.
/// 3. Keep characters allowed by [`AllowedCharacterSet`].
/// 4. Apply [`DotHandlingPolicy`].
/// 5. Convert all other character runs to one `replacement_delimiter`.
/// 6. Trim leading and trailing `replacement_delimiter` characters.
/// 7. Lowercase if `lowercase_enabled` is `true`.
/// 8. Apply `max_slug_chars` if present, then strip any trailing
///    `replacement_delimiter` the cut exposed.
/// 9. Apply [`EmptyOutputPolicy`] if the slug is empty.
/// 10. Validate according to [`SlugValidationPolicy`].
///
/// See the crate-level documentation (the README) for worked examples of each
/// target surface: default Unicode slugs, local path segments, URL path
/// segments, dot handling, and transliteration.
///
/// A caller-provided [`EmptyOutputPolicy::UseFallbackSlug`] value is inserted
/// verbatim — it is validated (step 10) but is not lowercased or truncated,
/// because steps 7 and 8 already ran on the empty slug it replaces.
pub fn slugify(input: &str, config: &SlugConfig) -> Result<SlugResult, SlugError> {
    let transliterated = apply_transliteration(input, config.transliteration_policy)?;
    let filtered = filter_chars(&transliterated, config);
    let trimmed = filtered.trim_matches(config.replacement_delimiter);
    let lowered = if config.lowercase_enabled {
        trimmed.to_lowercase()
    } else {
        trimmed.to_string()
    };
    let truncated = match config.max_slug_chars {
        Some(max_slug_chars) => {
            truncate_chars(lowered, max_slug_chars, config.replacement_delimiter)
        }
        None => lowered,
    };
    let slug = match (&config.empty_output_policy, truncated.is_empty()) {
        (EmptyOutputPolicy::UseFallbackSlug(fallback), true) => fallback.clone(),
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

fn filter_chars(input: &str, config: &SlugConfig) -> String {
    let mut output = String::with_capacity(input.len());
    let mut previous: Option<char> = None;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if is_allowed(ch, config.allowed_character_set) {
            output.push(ch);
        } else if ch == '.'
            && should_preserve_dot(previous, chars.peek().copied(), config.dot_handling_policy)
        {
            output.push('.');
        } else {
            push_replacement_delimiter(&mut output, config.replacement_delimiter);
        }
        previous = Some(ch);
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
fn truncate_chars(mut value: String, max_chars: usize, replacement_delimiter: char) -> String {
    if let Some((byte_index, _)) = value.char_indices().nth(max_chars) {
        value.truncate(byte_index);
    }
    while value.ends_with(replacement_delimiter) {
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

fn is_allowed(ch: char, allowed_character_set: AllowedCharacterSet) -> bool {
    match allowed_character_set {
        AllowedCharacterSet::UnicodeAlphanumericCharacters => ch.is_alphanumeric(),
        AllowedCharacterSet::AsciiAlphanumericCharacters => ch.is_ascii_alphanumeric(),
        AllowedCharacterSet::UnicodeLettersAndDecimalDigits => {
            is_unicode_letter(ch) || is_unicode_decimal_digit(ch)
        }
    }
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
                return Err(SlugError::PathSegmentWhitespace { character: ch })
            }
            _ if ch.is_control() => {
                return Err(SlugError::PathSegmentControlCharacter { character: ch })
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
        static MAP: &[(&str, &str)] = &[("a", "1"), ("abc", "9")];
        let config = SlugConfig {
            transliteration_policy: TransliterationPolicy::StaticReplacementMap(MAP),
            ..SlugConfig::default()
        };

        // "abc" matches both "a" and "abc" at index 0; the longer pattern wins
        // regardless of slice order.
        assert_eq!(slug("abc", &config), Ok("9".to_string()));
        assert_eq!(slug("ax", &config), Ok("1x".to_string()));
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

        let fallback_after_max = SlugConfig {
            max_slug_chars: Some(0),
            empty_output_policy: EmptyOutputPolicy::UseFallbackSlug("fallback".to_string()),
            ..SlugConfig::default()
        };
        assert_eq!(slug("abc", &fallback_after_max), Ok("fallback".to_string()));
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

        let invalid_fallback = SlugConfig {
            empty_output_policy: EmptyOutputPolicy::UseFallbackSlug("bad/fallback".to_string()),
            validation_policy: SlugValidationPolicy::LocalPathSegment,
            ..SlugConfig::default()
        };
        assert_eq!(
            slug("!!!", &invalid_fallback),
            Err(SlugError::PathSegmentSeparator { character: '/' })
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
}
