//! Transliteration, character filtering and bounded scalar truncation.
use super::validation::{is_combining_mark, keeps, should_preserve_dot};
use super::{DotHandlingPolicy, SlugConfig, SlugError, TransliterationPolicy};
use std::borrow::Cow;

pub(super) fn apply_transliteration(
    input: &str,
    policy: TransliterationPolicy,
) -> Result<Cow<'_, str>, SlugError> {
    let map = match policy {
        TransliterationPolicy::None => return Ok(Cow::Borrowed(input)),
        TransliterationPolicy::StaticReplacementMap(map) => map,
    };

    super::validation::validate_transliteration_map(map)?;
    if map.is_empty() {
        return Ok(Cow::Borrowed(input));
    }

    let mut output: Option<String> = None;
    let mut remaining = input;
    while !remaining.is_empty() {
        if let Some((pattern, replacement)) = map
            .iter()
            .filter(|(pattern, _)| remaining.starts_with(*pattern))
            .max_by_key(|(pattern, _)| pattern.len())
        {
            // Copy the untouched prefix only when a replacement is needed.
            let output = output.get_or_insert_with(|| {
                let mut output = String::with_capacity(input.len());
                output.push_str(&input[..input.len() - remaining.len()]);
                output
            });
            output.push_str(replacement);
            remaining = &remaining[pattern.len()..];
            continue;
        }

        let Some(ch) = remaining.chars().next() else {
            break;
        };
        if let Some(output) = &mut output {
            output.push(ch);
        }
        remaining = &remaining[ch.len_utf8()..];
    }

    Ok(output.map_or(Cow::Borrowed(input), Cow::Owned))
}

/// Apply the character and dot policies to cased input.
pub(super) fn filter_chars(input: &str, config: &SlugConfig) -> String {
    let mut output = String::with_capacity(input.len());
    let mut previous: Option<char> = None;
    let mut previous_kept = false;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        let kept = keeps(ch, previous_kept, config.allowed_character_set);
        if kept {
            output.push(ch);
        } else if ch == '.'
            && (config.dot_handling_policy == DotHandlingPolicy::PreserveAllDots
                || (previous_kept
                    && chars
                        .peek()
                        .is_some_and(|next| keeps(*next, false, config.allowed_character_set))))
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
pub(super) fn truncate_chars(
    mut value: String,
    max_chars: usize,
    replacement_delimiter: char,
    dots: DotHandlingPolicy,
) -> String {
    let mut cluster_start = 0;
    let mut cutoff = None;
    for (count, (offset, ch)) in value.char_indices().enumerate() {
        let continues_cluster = ch != replacement_delimiter && is_combining_mark(ch);
        if count == max_chars {
            cutoff = Some(if continues_cluster {
                cluster_start
            } else {
                offset
            });
            break;
        }
        if !continues_cluster {
            cluster_start = offset;
        }
    }
    if let Some(cutoff) = cutoff {
        value.truncate(cutoff);
    }
    while value.ends_with(replacement_delimiter)
        || (dots == DotHandlingPolicy::PreserveDotsBetweenDecimalDigits && value.ends_with('.'))
    {
        value.pop();
    }
    value
}
