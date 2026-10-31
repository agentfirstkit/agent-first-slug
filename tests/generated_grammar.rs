//! Every slug the pipeline generates is one the configuration accepts back as
//! its own fallback, and slugifying it again changes nothing. A fallback is
//! held to "the grammar the pipeline would have produced", so a generated slug
//! the fallback check refuses means the two have drifted apart.

use agent_first_slug::{
    AllowedCharacterSet, DotHandlingPolicy, EmptyOutputPolicy, SlugConfig, slugify,
};

fn configurations() -> Vec<SlugConfig> {
    let mut configurations = Vec::new();
    for allowed_character_set in [
        AllowedCharacterSet::UnicodeAlphanumericCharacters,
        AllowedCharacterSet::AsciiAlphanumericCharacters,
        AllowedCharacterSet::UnicodeLettersAndDecimalDigits,
        AllowedCharacterSet::UnicodeLettersMarksAndDecimalDigits,
    ] {
        for dot_handling_policy in [
            DotHandlingPolicy::ReplaceAllDots,
            DotHandlingPolicy::PreserveAllDots,
            DotHandlingPolicy::PreserveDotsBetweenDecimalDigits,
        ] {
            for lowercase_enabled in [true, false] {
                for max_slug_chars in [None, Some(0), Some(1), Some(2), Some(3), Some(5)] {
                    for replacement_delimiter in ['-', '_', '\u{301}'] {
                        configurations.push(SlugConfig {
                            replacement_delimiter,
                            lowercase_enabled,
                            max_slug_chars,
                            allowed_character_set,
                            dot_handling_policy,
                            ..SlugConfig::default()
                        });
                    }
                }
            }
        }
    }
    configurations
}

#[test]
fn generated_slugs_satisfy_the_fallback_grammar_and_are_stable() {
    // Case expansion, Arabic-Indic digits, orphaned and attached marks, final
    // sigma, titlecase, and the punctuation the delimiters and dots meet.
    let alphabet: Vec<char> = "aA1.. -_İ١٢\u{301}\u{E48}ไมन्\u{93E}ǅßẞΣ\u{345}ΐ"
        .chars()
        .collect();
    let configurations = configurations();
    // A fixed xorshift seed keeps the run deterministic.
    let mut state: u64 = 0x2545_f491_4f6c_dd1d;
    let mut next = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        usize::try_from(state % 1_000_003).unwrap_or_default()
    };
    for _ in 0..50_000 {
        let input: String = (0..next() % 9)
            .map(|_| alphabet[next() % alphabet.len()])
            .collect();
        let config = &configurations[next() % configurations.len()];
        // The marks set refuses a mark as its delimiter; that is covered elsewhere.
        let Ok(generated) = slugify(&input, config).map(|result| result.slug) else {
            continue;
        };
        if generated.is_empty() {
            continue;
        }
        let as_fallback = SlugConfig {
            empty_output_policy: EmptyOutputPolicy::UseFallbackSlug(generated.clone()),
            ..config.clone()
        };
        assert_eq!(
            slugify("", &as_fallback).map(|result| result.slug),
            Ok(generated.clone()),
            "{input:?} under {config:?}"
        );
        assert_eq!(
            slugify(&generated, config).map(|result| result.slug),
            Ok(generated.clone()),
            "{input:?} under {config:?}"
        );
    }
}
