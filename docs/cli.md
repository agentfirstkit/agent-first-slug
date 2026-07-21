<!-- Generated. Do not edit by hand. Regenerate: afslug --help --recursive --output markdown -->

# afslug CLI Reference

# afslug - Generate and validate slugs with explicit agent-first-slug rules.

```text
Usage: afslug [OPTIONS] <COMMAND>

Commands:
  slugify   Generate a slug from input text
  validate  Validate an existing value as a path segment
  skill     Manage Agent-First Slug skills for Codex, Claude Code, opencode, and Hermes
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

## afslug skill - Manage Agent-First Slug skills for Codex, Claude Code, opencode, and Hermes

```text
Usage: skill <COMMAND>

Commands:
  status     Show whether the Agent-First Slug skill is installed, valid, and up to date
  install    Install the Agent-First Slug skill
  uninstall  Remove an afslug-managed Agent-First Slug skill
  help       Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help
```

### afslug skill status - Show whether the Agent-First Slug skill is installed, valid, and up to date

```text
Usage: status [OPTIONS]

Options:
      --agent <AGENT>
          Agent to manage. Defaults to all personal skill targets

          Possible values:
          - all:         Manage every agent that supports the requested scope
          - codex:       Codex under $CODEX_HOME/skills
          - claude-code: Claude Code under ~/.claude/skills or .claude/skills
          - opencode:    opencode under ~/.config/opencode/skills or .opencode/skills
          - hermes:      Hermes under $HERMES_HOME/skills or ~/.hermes/skills

          [default: all]

      --scope <SCOPE>
          Skill scope

          Possible values:
          - personal:  Install under the user-level skills directory
          - workspace: Install under the current workspace's skills directory

          [default: personal]

      --skills-dir <SKILLS_DIR>
          Directory that contains skill folders. Requires an explicit single --agent

  -h, --help
          Print help (see a summary with '-h')
```

### afslug skill install - Install the Agent-First Slug skill

```text
Usage: install [OPTIONS]

Options:
      --agent <AGENT>
          Agent to manage. Defaults to all personal skill targets

          Possible values:
          - all:         Manage every agent that supports the requested scope
          - codex:       Codex under $CODEX_HOME/skills
          - claude-code: Claude Code under ~/.claude/skills or .claude/skills
          - opencode:    opencode under ~/.config/opencode/skills or .opencode/skills
          - hermes:      Hermes under $HERMES_HOME/skills or ~/.hermes/skills

          [default: all]

      --scope <SCOPE>
          Skill scope

          Possible values:
          - personal:  Install under the user-level skills directory
          - workspace: Install under the current workspace's skills directory

          [default: personal]

      --skills-dir <SKILLS_DIR>
          Directory that contains skill folders. Requires an explicit single --agent

      --force
          Overwrite or remove an unmanaged Agent-First Slug skill at the target path

  -h, --help
          Print help (see a summary with '-h')
```

### afslug skill uninstall - Remove an afslug-managed Agent-First Slug skill

```text
Usage: uninstall [OPTIONS]

Options:
      --agent <AGENT>
          Agent to manage. Defaults to all personal skill targets

          Possible values:
          - all:         Manage every agent that supports the requested scope
          - codex:       Codex under $CODEX_HOME/skills
          - claude-code: Claude Code under ~/.claude/skills or .claude/skills
          - opencode:    opencode under ~/.config/opencode/skills or .opencode/skills
          - hermes:      Hermes under $HERMES_HOME/skills or ~/.hermes/skills

          [default: all]

      --scope <SCOPE>
          Skill scope

          Possible values:
          - personal:  Install under the user-level skills directory
          - workspace: Install under the current workspace's skills directory

          [default: personal]

      --skills-dir <SKILLS_DIR>
          Directory that contains skill folders. Requires an explicit single --agent

      --force
          Overwrite or remove an unmanaged Agent-First Slug skill at the target path

  -h, --help
          Print help (see a summary with '-h')
```
AFDATA: 0.22.0
