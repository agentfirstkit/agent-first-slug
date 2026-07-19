<!-- Generated. Do not edit by hand. Regenerate: afslug --help --recursive --output markdown -->

# afslug CLI Reference

# afslug - Generate and validate slugs with explicit agent-first-slug rules.

```text
Usage: afslug [OPTIONS] <COMMAND>

Commands:
  slugify   Generate a slug from input text
  validate  Validate an existing value as a path segment
  help      Print this message or the help of the given subcommand(s)

Options:
      --output <OUTPUT>
          Output format: json, yaml, or plain

          [default: json]

  -V, --version
          Print the CLI version

  -h, --help
          Print help. Add --recursive to expand every nested subcommand; add --output json|yaml|markdown to render this help in another format.
```

## afslug slugify - Generate a slug from input text

```text
Usage: slugify [OPTIONS] <INPUT>

Arguments:
  <INPUT>
          Text to slugify

Options:
      --delimiter <DELIMITER>
          Delimiter inserted for each run of filtered characters

          [default: -]

      --no-lowercase
          Keep the original case instead of lowercasing the slug

      --max-chars <N>
          Cap the slug to at most N Unicode characters

      --charset <CHARSET>
          Character set kept from the input after filtering

          Possible values:
          - unicode-alphanumeric:   Unicode alphanumeric characters
          - ascii-alphanumeric:     ASCII letters and digits only
          - unicode-letters-digits: Unicode letters plus decimal digits

          [default: unicode-alphanumeric]

      --dots <DOTS>
          How input dots are handled before other characters become delimiters

          Possible values:
          - replace:                 Treat every dot as a delimiter
          - preserve:                Preserve every dot
          - preserve-between-digits: Preserve a dot only between two decimal digits

          [default: replace]

      --validation <VALIDATION>
          Validation applied to the generated slug

          Possible values:
          - none:       No validation
          - local-path: Validate as one local filesystem path segment
          - url-path:   Validate as one URL path segment

          [default: none]

      --fallback <SLUG>
          Slug substituted when the generated slug would otherwise be empty

  -h, --help
          Print help (see a summary with '-h')
```

## afslug validate - Validate an existing value as a path segment

```text
Usage: validate [OPTIONS] <VALUE>

Arguments:
  <VALUE>
          Value to validate as a path segment

Options:
      --policy <POLICY>
          Path-segment kind to validate against

          Possible values:
          - local-path: One local filesystem path segment
          - url-path:   One URL path segment

          [default: local-path]

  -h, --help
          Print help (see a summary with '-h')
```
AFDATA: 0.19.1
