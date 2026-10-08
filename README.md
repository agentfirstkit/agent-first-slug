# Agent-First Slug

Rust slug generation with explicit caller configuration for path and URL path segments.

> **Ask your agent:** "Add agent-first-slug to my project and use it to slugify titles for URL path segments."

Start by choosing the target surface: a local filesystem path segment, a URL path
segment, or a legacy slug format you need to preserve. The examples below show
complete config values so the behavior is visible at the call site.

## Install the Library

```bash
cargo add agent-first-slug --no-default-features
```

## Install the CLI

```bash
# prebuilt binary
brew install agentfirstkit/tap/afslug   # macOS / Linux
scoop bucket add agentfirstkit https://github.com/agentfirstkit/scoop-bucket && scoop install afslug   # Windows

# or from crates.io
cargo install agent-first-slug
```

Prebuilt archives are also available from
[GitHub Releases](https://github.com/agentfirstkit/agent-first-slug/releases).

## CLI

`afslug` generates and validates slugs, emitting one AFDATA protocol event per
run. JSON is the default; YAML and plain output are also available.

```bash
afslug slugify "Hello, 世界!"
# {"kind":"result","result":{"changed_from_input":true,"code":"slugify","slug":"hello-世界"},"trace":{}}

afslug slugify "Hello, World!" --output plain
# kind=result result.changed_from_input=true result.code=slugify result.slug=hello-world trace={}

afslug validate "my-slug" --policy url-path
```

`slugify` exposes the [`SlugConfig`](#default-unicode-slugs) surface as flags —
delimiter, case, truncation, character set, dot handling, validation, and an
empty-slug fallback (`afslug slugify --help` lists them); `validate` checks an
existing value as a local or URL path segment. Transliteration stays
library-only: its static replacement map cannot be built from CLI arguments.

## Agent Skill

Use [`skills/agent-first-slug/SKILL.md`](skills/agent-first-slug/SKILL.md) to
teach a coding agent when to choose the Rust library or the explicitly configured
`afslug` CLI, and how to preserve stable identifier behavior.

## Default Unicode Slugs

```rust
use agent_first_slug::{slugify, SlugConfig};

let config = SlugConfig::default();

assert_eq!(slugify("Hello", &config)?.slug, "hello");
assert_eq!(slugify("Hello World", &config)?.slug, "hello-world");
assert_eq!(slugify("Hello,  world!!!", &config)?.slug, "hello-world");
assert_eq!(slugify("-- already -- spaced --", &config)?.slug, "already-spaced");
assert_eq!(slugify("!!!", &config)?.slug, "");
# Ok::<(), agent_first_slug::SlugError>(())
```

Unicode letters and numbers are preserved by default:

```rust
use agent_first_slug::{slugify, SlugConfig};

let config = SlugConfig::default();

assert_eq!(
    slugify("現在的Nobody，未來的Somebody！", &config)?.slug,
    "現在的nobody-未來的somebody"
);
assert_eq!(
    slugify("牙好，胃口就好，身体倍儿棒，吃嘛嘛香。", &config)?.slug,
    "牙好-胃口就好-身体倍儿棒-吃嘛嘛香"
);
assert_eq!(slugify("お元気ですか？", &config)?.slug, "お元気ですか");
# Ok::<(), agent_first_slug::SlugError>(())
```

## Local Filesystem Path Segment

Use this when the slug will be one segment in a local path. Slugify each segment
separately; do not pass a full path through one slug call.

Requirements for this target:

- Reject empty output unless the caller provides a fallback.
- Reject `/`, `\\`, Unicode whitespace, and control characters.
- Reject `.` and `..` to avoid current-directory and parent-directory meanings.
- Prefer replacing dots unless the caller explicitly wants dots in filenames.
- Choose ASCII-only if the path must be portable across legacy filesystems or tools.

```rust
use agent_first_slug::{
    slugify, AllowedCharacterSet, DotHandlingPolicy, EmptyOutputPolicy, SlugConfig,
    SlugValidationPolicy, TransliterationPolicy,
};

let config = SlugConfig {
    replacement_delimiter: '-',
    lowercase_enabled: true,
    max_slug_chars: None,
    allowed_character_set: AllowedCharacterSet::UnicodeAlphanumericCharacters,
    dot_handling_policy: DotHandlingPolicy::ReplaceAllDots,
    transliteration_policy: TransliterationPolicy::None,
    validation_policy: SlugValidationPolicy::LocalPathSegment,
    empty_output_policy: EmptyOutputPolicy::UseFallbackSlug("fallback".to_string()),
};

assert_eq!(slugify("Ubuntu 16.04", &config)?.slug, "ubuntu-16-04");
assert_eq!(slugify("你好，世界", &config)?.slug, "你好-世界");
assert_eq!(slugify("!!!", &config)?.slug, "fallback");
# Ok::<(), agent_first_slug::SlugError>(())
```

ASCII-only path segment with fallback:

```rust
use agent_first_slug::{
    slugify, AllowedCharacterSet, DotHandlingPolicy, EmptyOutputPolicy, SlugConfig,
    SlugValidationPolicy, TransliterationPolicy,
};

let config = SlugConfig {
    replacement_delimiter: '-',
    lowercase_enabled: true,
    max_slug_chars: None,
    allowed_character_set: AllowedCharacterSet::AsciiAlphanumericCharacters,
    dot_handling_policy: DotHandlingPolicy::ReplaceAllDots,
    transliteration_policy: TransliterationPolicy::None,
    validation_policy: SlugValidationPolicy::LocalPathSegment,
    empty_output_policy: EmptyOutputPolicy::UseFallbackSlug("fallback".to_string()),
};

assert_eq!(slugify("Hello 世界", &config)?.slug, "hello");
assert_eq!(slugify("Ubuntu 16.04", &config)?.slug, "ubuntu-16-04");
assert_eq!(slugify("你好，世界", &config)?.slug, "fallback");
# Ok::<(), agent_first_slug::SlugError>(())
```

## URL Path Segment

Use this when the slug will be one segment in a URL path. The returned slug is raw
UTF-8; percent-encode it or use a URL library's path-segment API when building the
final URL.

Requirements for this target:

- Reject empty output unless the caller provides a fallback.
- Reject `/`, `?`, `#`, raw `%`, Unicode whitespace, controls, and `\\`.
- Reject `.` and `..` for route safety.
- Preserve dots between decimal digits if version numbers matter.
- For scripts that write vowels, viramas, or tones as combining marks, choose
  `UnicodeLettersMarksAndDecimalDigits`; see [Combining Marks](#combining-marks).
- Do not manually concatenate unescaped slugs into URLs.

```rust
use agent_first_slug::{
    slugify, AllowedCharacterSet, DotHandlingPolicy, EmptyOutputPolicy, SlugConfig,
    SlugValidationPolicy, TransliterationPolicy,
};

let config = SlugConfig {
    replacement_delimiter: '-',
    lowercase_enabled: true,
    max_slug_chars: None,
    allowed_character_set: AllowedCharacterSet::UnicodeLettersAndDecimalDigits,
    dot_handling_policy: DotHandlingPolicy::PreserveDotsBetweenDecimalDigits,
    transliteration_policy: TransliterationPolicy::None,
    validation_policy: SlugValidationPolicy::UrlPathSegment,
    empty_output_policy: EmptyOutputPolicy::KeepEmptySlug,
};

assert_eq!(slugify("Ubuntu 16.04", &config)?.slug, "ubuntu-16.04");
assert_eq!(slugify("T.U.S.F.G.E.3.0.8", &config)?.slug, "t-u-s-f-g-e-3.0.8");
assert_eq!(slugify(".18 increased ! ", &config)?.slug, "18-increased");
assert_eq!(slugify("お元気ですか？", &config)?.slug, "お元気ですか");
# Ok::<(), agent_first_slug::SlugError>(())
```

## Combining Marks

Thai, Lao, Devanagari, Tamil and many other scripts write vowels, viramas, or
tones as combining marks after a base letter. Only
`UnicodeLettersMarksAndDecimalDigits` keeps those words whole.
`UnicodeLettersAndDecimalDigits` turns every mark into a delimiter, and the
default `UnicodeAlphanumericCharacters` keeps the marks Unicode calls
alphabetic — most vowel signs — but still splits at viramas and tone marks.

```rust
use agent_first_slug::{slugify, AllowedCharacterSet, SlugConfig};

let marks = SlugConfig {
    allowed_character_set: AllowedCharacterSet::UnicodeLettersMarksAndDecimalDigits,
    ..SlugConfig::default()
};

assert_eq!(slugify("नमस्ते ไม่ใช่", &marks)?.slug, "नमस्ते-ไม่ใช่");
assert_eq!(slugify("नमस्ते ไม่ใช่", &SlugConfig::default())?.slug, "नमस-ते-ไม-ใช");
# Ok::<(), agent_first_slug::SlugError>(())
```

A mark is kept only where it completes a kept character. One with nothing kept
before it — at the start, after a delimiter, after a dot — is filtered like any
other character, so a slug never begins with a stray mark.

## Dot Handling

```rust
use agent_first_slug::{slugify, DotHandlingPolicy, SlugConfig};

let replace_all_dots = SlugConfig {
    dot_handling_policy: DotHandlingPolicy::ReplaceAllDots,
    ..SlugConfig::default()
};
let preserve_all_dots = SlugConfig {
    dot_handling_policy: DotHandlingPolicy::PreserveAllDots,
    ..SlugConfig::default()
};
let preserve_version_dots = SlugConfig {
    dot_handling_policy: DotHandlingPolicy::PreserveDotsBetweenDecimalDigits,
    ..SlugConfig::default()
};

assert_eq!(slugify("Ubuntu 16.04", &replace_all_dots)?.slug, "ubuntu-16-04");
assert_eq!(slugify("A.B..C", &preserve_all_dots)?.slug, "a.b..c");
assert_eq!(slugify("T.U.S.F.G.E.3.0.8", &preserve_version_dots)?.slug, "t-u-s-f-g-e-3.0.8");
# Ok::<(), agent_first_slug::SlugError>(())
```

## Transliteration

Transliteration is caller-provided, so legacy behavior can be expressed without a
named preset in the library. Patterns must be nonempty and unique, even when
duplicate patterns name the same replacement. Invalid maps are refused even
for empty input; unique overlapping patterns still use the longest match.

```rust
use agent_first_slug::{
    slugify, AllowedCharacterSet, SlugConfig, TransliterationPolicy,
};

static MAP: &[(&str, &str)] = &[("Æ", "AE"), ("東京", "Tokyo")];
let config = SlugConfig {
    allowed_character_set: AllowedCharacterSet::AsciiAlphanumericCharacters,
    transliteration_policy: TransliterationPolicy::StaticReplacementMap(MAP),
    ..SlugConfig::default()
};

assert_eq!(slugify("Æther 東京", &config)?.slug, "aether-tokyo");
# Ok::<(), agent_first_slug::SlugError>(())
```

## Truncation And Empty Output

`max_slug_chars` counts Unicode scalar values. A cut never separates a kept
character from the combining marks after it — that character is dropped whole —
and any trailing delimiter the cut exposes is then stripped, including a
trailing dot under `PreserveDotsBetweenDecimalDigits`. Empty handling runs
after truncation.

A fallback is inserted as written rather than run through the pipeline, but
`UseFallbackSlug` still requires it to satisfy the same configuration —
character set, delimiter, dot policy, case and length. A fallback that does not
is a configuration error, not a slug: without that check an ASCII-only,
length-capped configuration could return an arbitrary Unicode string and report
it as validated. `UseVerbatimFallbackSlug` waives it for a value that has to
match something already stored, and checks only the target surface.

```rust
use agent_first_slug::{slugify, EmptyOutputPolicy, SlugConfig, SlugError};

let truncated = SlugConfig {
    max_slug_chars: Some(8),
    ..SlugConfig::default()
};
let fallback = SlugConfig {
    max_slug_chars: Some(8),
    empty_output_policy: EmptyOutputPolicy::UseFallbackSlug("untitled".to_string()),
    ..SlugConfig::default()
};
let legacy = SlugConfig {
    max_slug_chars: Some(8),
    empty_output_policy: EmptyOutputPolicy::UseVerbatimFallbackSlug(
        "Legacy Name".to_string(),
    ),
    ..SlugConfig::default()
};

assert_eq!(slugify("Long Example", &truncated)?.slug, "long-exa");
assert_eq!(slugify("!!!", &fallback)?.slug, "untitled");
assert_eq!(slugify("!!!", &legacy)?.slug, "Legacy Name");

// Too long for this configuration's own budget, so it is refused rather than
// returned as if it had been generated.
let over_budget = SlugConfig {
    empty_output_policy: EmptyOutputPolicy::UseFallbackSlug("far-too-long".to_string()),
    ..fallback
};
assert!(matches!(
    slugify("!!!", &over_budget),
    Err(SlugError::FallbackViolatesConfig { .. })
));
# Ok::<(), agent_first_slug::SlugError>(())
```

## What Changed, And What To Check Before Upgrading

Three rules that can change stored slugs or reject a configuration changed in
0.8. If you have slugs already stored, run the old and new versions over your
corpus and diff before upgrading.

**Truncation no longer leaves a broken tail.** A cut that exposed a trailing
dot under `PreserveDotsBetweenDecimalDigits` kept it (`version 1.2.3` at ten
scalars was `version-1.`, now `version-1`), and a cut between a character and
its combining marks kept the bare base (`नमस्ते` at five scalars was `नमस-त`,
now `नमस`).

**Repeated transliteration patterns are refused**
(`SlugError::DuplicateTransliterationPattern`), even when both entries name the
same replacement, and even for empty input.

**A fallback that repeats the delimiter is refused** under `UseFallbackSlug`,
because the pipeline never produces one.

Nothing normalizes input — see the skill for what that means for a
stable-identifier contract.

## Validation Only

```rust
use agent_first_slug::{validate_slug, SlugError, SlugValidationPolicy};

assert_eq!(validate_slug("safe-name", SlugValidationPolicy::LocalPathSegment), Ok(()));
assert_eq!(
    validate_slug("a/b", SlugValidationPolicy::LocalPathSegment),
    Err(SlugError::PathSegmentSeparator { character: '/' })
);
assert_eq!(
    validate_slug("a?b", SlugValidationPolicy::UrlPathSegment),
    Err(SlugError::UrlPathSegmentReservedCharacter { character: '?' })
);
```

## License

MIT
