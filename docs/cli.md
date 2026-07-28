<!-- Generated. Do not edit by hand. Regenerate: afslug --help --recursive --output markdown -->

# afslug CLI Reference

# afslug - Generate and validate slugs with explicit agent-first-slug rules.

```text
afslug [OPTIONS] <COMMAND>
```

| Argument | Description |
| --- | --- |
| `--output <OUTPUT>` | Output format: json, yaml, or plain *(global, default: `json`)* |
| `--output-to <OUTPUT_TO>` | Output routing: split, stdout, or stderr *(global, default: `split`)* |
| `--version` | Print the CLI version |
| `--help` | Print help. Add --recursive to expand every nested subcommand; add --output plain\|json\|yaml\|markdown to choose the format. |

| Command | Summary |
| --- | --- |
| `afslug slugify` | Generate a slug from input text |
| `afslug validate` | Validate an existing value as a path segment |
| `afslug skill` | Manage Agent-First Slug skills for Codex, Claude Code, opencode, and Hermes |

## afslug slugify - Generate a slug from input text

```text
afslug slugify [OPTIONS] <INPUT>
```

| Argument | Description |
| --- | --- |
| `INPUT` | Text to slugify *(required)* |
| `--delimiter <DELIMITER>` | Delimiter inserted for each run of filtered characters *(default: `-`)* |
| `--no-lowercase` | Keep the original case instead of lowercasing the slug |
| `--max-chars <N>` | Cap the slug to at most N Unicode characters |
| `--charset <CHARSET>` | Character set kept from the input after filtering *(default: `unicode-alphanumeric`)* |
| `--dots <DOTS>` | How input dots are handled before other characters become delimiters *(default: `replace`)* |
| `--validation <VALIDATION>` | Validation applied to the generated slug *(default: `none`)* |
| `--fallback <SLUG>` | Slug substituted when the generated slug would otherwise be empty |

## afslug validate - Validate an existing value as a path segment

```text
afslug validate [OPTIONS] <VALUE>
```

| Argument | Description |
| --- | --- |
| `VALUE` | Value to validate as a path segment *(required)* |
| `--policy <POLICY>` | Path-segment kind to validate against *(default: `local-path`)* |

## afslug skill - Manage Agent-First Slug skills for Codex, Claude Code, opencode, and Hermes

```text
afslug skill <COMMAND>
```

| Command | Summary |
| --- | --- |
| `afslug skill status` | Show whether the Agent-First Slug skill is installed, valid, and up to date |
| `afslug skill install` | Install the Agent-First Slug skill |
| `afslug skill uninstall` | Remove an afslug-managed Agent-First Slug skill |

### afslug skill status - Show whether the Agent-First Slug skill is installed, valid, and up to date

```text
afslug skill status [OPTIONS]
```

| Argument | Description |
| --- | --- |
| `--agent <AGENT>` | Agent to manage. Defaults to all personal skill targets *(default: `all`)* |
| `--scope <SCOPE>` | Skill scope *(default: `personal`)* |
| `--skills-dir <SKILLS_DIR>` | Directory that contains skill folders. Requires an explicit single --agent |

### afslug skill install - Install the Agent-First Slug skill

```text
afslug skill install [OPTIONS]
```

| Argument | Description |
| --- | --- |
| `--agent <AGENT>` | Agent to manage. Defaults to all personal skill targets *(default: `all`)* |
| `--scope <SCOPE>` | Skill scope *(default: `personal`)* |
| `--skills-dir <SKILLS_DIR>` | Directory that contains skill folders. Requires an explicit single --agent |
| `--force` | Overwrite or remove an unmanaged Agent-First Slug skill at the target path |

### afslug skill uninstall - Remove an afslug-managed Agent-First Slug skill

```text
afslug skill uninstall [OPTIONS]
```

| Argument | Description |
| --- | --- |
| `--agent <AGENT>` | Agent to manage. Defaults to all personal skill targets *(default: `all`)* |
| `--scope <SCOPE>` | Skill scope *(default: `personal`)* |
| `--skills-dir <SKILLS_DIR>` | Directory that contains skill folders. Requires an explicit single --agent |
| `--force` | Overwrite or remove an unmanaged Agent-First Slug skill at the target path |
