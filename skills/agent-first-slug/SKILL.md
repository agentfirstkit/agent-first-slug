---
name: agent-first-slug
description: Use agent-first-slug to generate deterministic Unicode slugs, local filesystem path segments, or URL path segments from Rust, and use the afslug CLI for one-off default slug generation with AFDATA JSON/YAML/plain output. Trigger when adding, reviewing, migrating, or validating slug, filename-segment, route-segment, dot-handling, transliteration, truncation, or empty-output behavior.
---

# Agent-First Slug

Choose the Rust library when behavior must be configured or embedded in an
application. Choose `afslug` for one-off generation with `SlugConfig::default()`.
Do not invent downstream presets or silently change an existing identifier
scheme; make compatibility-affecting rules explicit.

## Library workflow

Install the lightweight Rust library without the CLI dependencies:

```bash
cargo add agent-first-slug --no-default-features
```

Use the default Unicode rules only when they match the target contract:

```rust
use agent_first_slug::{slugify, SlugConfig};

let result = slugify("Hello, 世界!", &SlugConfig::default())?;
assert_eq!(result.slug, "hello-世界");
# Ok::<(), agent_first_slug::SlugError>(())
```

For a local path or URL path segment, construct every `SlugConfig` field at the
call site. Select the character set, dot policy, truncation, empty-output
policy, and validation policy deliberately. Transliteration is a caller-owned
static replacement map; the library does not guess a language or legacy mode.

```rust
use agent_first_slug::{
    slugify, AllowedCharacterSet, DotHandlingPolicy, EmptyOutputPolicy,
    SlugConfig, SlugValidationPolicy, TransliterationPolicy,
};

let config = SlugConfig {
    replacement_delimiter: '-',
    lowercase_enabled: true,
    max_slug_chars: Some(80),
    allowed_character_set: AllowedCharacterSet::UnicodeLettersAndDecimalDigits,
    dot_handling_policy: DotHandlingPolicy::PreserveDotsBetweenDecimalDigits,
    transliteration_policy: TransliterationPolicy::None,
    validation_policy: SlugValidationPolicy::UrlPathSegment,
    empty_output_policy: EmptyOutputPolicy::UseFallbackSlug("item".into()),
};
let slug = slugify("Ubuntu 16.04", &config)?.slug;
assert_eq!(slug, "ubuntu-16.04");
# Ok::<(), agent_first_slug::SlugError>(())
```

- Slugify one segment at a time; never pass a full filesystem path or URL.
- Treat the returned URL segment as raw UTF-8. Percent-encode it with a URL
  library's path-segment API before constructing the final URL.
- A fallback is inserted verbatim and then validated; it is not lowercased or
  truncated. Choose a fallback that already satisfies the target contract.
- Changing delimiter, case, transliteration, dots, character set, or truncation
  can change stable identifiers. Report migration impact before changing stored
  or public slugs.

## CLI workflow

Install the standalone command with its default `cli` feature:

```bash
cargo install agent-first-slug
```

Run `afslug slugify <INPUT>` to generate a slug or `afslug validate <VALUE>` to
check an existing value, and parse the terminal AFDATA event together with the
exit status:

```bash
afslug slugify "Hello, 世界!"
# {"kind":"result","result":{"changed_from_input":true,"code":"slugify","slug":"hello-世界"},"trace":{}}

afslug slugify "Hello, World!" --output plain
# kind=result result.changed_from_input=true result.code=slugify result.slug=hello-world

afslug validate "my-slug" --policy url-path
```

`--output` accepts `json`, `yaml`, or `plain`. Bare `--version` is conventional
human text; `--version --output json|yaml|plain` is structured. Argument and
slug failures are AFDATA error events on stdout with nonzero exit status.

`slugify` sets the `SlugConfig` surface through flags (`afslug slugify --help`
enumerates them); pass every flag the target contract needs rather than
post-processing the slug to simulate a different policy. Transliteration is the
one policy the CLI cannot express — its static replacement map only exists in
the library — so reach for the crate when you need it.

## Verification

- Test unchanged input, punctuation-only input, Unicode, dots, empty output,
  truncation boundaries, fallbacks, and every selected validation policy.
- For stable identifiers, add golden tests before refactoring a config.
- For CLI changes, verify JSON/YAML/plain success, structured errors, and
  explicit structured version output through the repository test script.
