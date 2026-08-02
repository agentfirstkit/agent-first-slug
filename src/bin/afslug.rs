use std::process::ExitCode;

use agent_first_data::skill::{
    self, SkillAction, SkillAgentSelection, SkillAsset, SkillOptions, SkillScope, SkillSpec,
};
use agent_first_data::{
    ArgSpec, BuiltCliSpec, CliEmitter, CliOutcome, CliSpec, CliSpecError, CliValue, Combination,
    CommandSpec, OutputFormat, OutputPlan, OutputSpec, OutputTo, ResolvedInvocation,
    build_afdata_cli, cli_error_event, cli_help_event, cli_parse_output, cli_version_event,
    render_cli_reference,
};
use agent_first_slug::{
    AllowedCharacterSet, DotHandlingPolicy, EmptyOutputPolicy, SlugConfig, SlugResult,
    SlugValidationPolicy, TransliterationPolicy, slugify, validate_slug,
};
use serde_json::{Value, json};

const SKILL_SPEC: SkillSpec = SkillSpec {
    name: "agent-first-slug",
    source: include_str!("../../skills/agent-first-slug/SKILL.md"),
    title: "Agent-First Slug",
    marker_slug: "afslug",
    // SKILL.md ships with an OpenAI/Codex agent interface file; it installs
    // alongside SKILL.md as a bundled asset.
    assets: &[SkillAsset {
        path: "agents/openai.yaml",
        contents: include_str!("../../skills/agent-first-slug/agents/openai.yaml"),
    }],
};

/// The named agents, and the fan-out value that is not one of them. The two
/// shapes of every skill verb partition `--agent` between them, so both the
/// enum and that partition are built from this one list.
const AGENTS: [&str; 4] = ["codex", "claude-code", "opencode", "hermes"];
const EVERY_AGENT: &str = "all";

fn output() -> OutputSpec {
    OutputSpec::protocol_finite(
        ["json", "yaml", "plain"],
        ["split", "stdout", "stderr"],
        "json",
        "split",
    )
}

/// The whole CLI: one registry that is the single source for argv parsing,
/// typed values, legal argument combinations, help, and `docs/cli.md`.
fn cli_spec() -> Result<BuiltCliSpec, CliSpecError> {
    let mut spec =
        CliSpec::new("afslug", env!("CARGO_PKG_VERSION"))
            .about("Generate and validate slugs with explicit agent-first-slug rules.")
            .display_name(env!("DISPLAY_NAME"))
            .lifecycle_output(output())
            .command(CommandSpec::root())
            .command(slugify_command())
            .command(validate_command())
            .command(CommandSpec::new(["skill"]).about(
                "Manage Agent-First Slug skills for Codex, Claude Code, opencode, and Hermes.",
            ))
            .command(skill_command(
                "status",
                "Show whether the Agent-First Slug skill is installed, valid, and up to date.",
                false,
            ))
            .command(skill_command(
                "install",
                "Install the Agent-First Slug skill.",
                true,
            ))
            .command(skill_command(
                "uninstall",
                "Remove an afslug-managed Agent-First Slug skill.",
                true,
            ));
    // Absent from a source tarball with no reachable .git, and the version
    // payload omits it rather than reporting the literal "unknown".
    if let Some(build) = Some(env!("GIT_SHA")).filter(|sha| *sha != "unknown") {
        spec = spec.build_id(build);
    }
    build_afdata_cli(spec)
}

fn slugify_command() -> CommandSpec {
    // Transliteration is intentionally absent: its policy carries a `'static`
    // replacement map that a CLI cannot build from runtime input, so callers who
    // need it reach for the library.
    CommandSpec::new(["slugify"])
        .about("Generate a slug from input text.")
        .arg(ArgSpec::positional("input", 0, "TEXT").about("Text to slugify"))
        .arg(
            ArgSpec::option("--delimiter", "CHAR")
                .default("-")
                .about("Delimiter inserted for each run of filtered characters"),
        )
        .arg(
            ArgSpec::flag("--no-lowercase")
                .about("Keep the original case instead of lowercasing the slug"),
        )
        .arg(
            ArgSpec::option_i64("--max-chars", "N")
                .about("Cap the slug to at most N Unicode characters"),
        )
        .arg(
            ArgSpec::option_enum(
                "--charset",
                [
                    "unicode-alphanumeric",
                    "ascii-alphanumeric",
                    "unicode-letters-digits",
                ],
            )
            .value_name("CHARSET")
            .default("unicode-alphanumeric")
            .about("Character set kept from the input after filtering"),
        )
        .arg(
            ArgSpec::option_enum("--dots", ["replace", "preserve", "preserve-between-digits"])
                .value_name("DOTS")
                .default("replace")
                .about("How input dots are handled before other characters become delimiters"),
        )
        .arg(
            ArgSpec::option_enum("--validation", ["none", "local-path", "url-path"])
                .value_name("VALIDATION")
                .default("none")
                .about("Validation applied to the generated slug"),
        )
        .arg(
            ArgSpec::option("--fallback", "SLUG")
                .about("Slug substituted when the generated slug would otherwise be empty"),
        )
        .combination(
            Combination::new("slugify")
                .action("slugify")
                .required(["input"])
                .optional([
                    "delimiter",
                    "no_lowercase",
                    "max_chars",
                    "charset",
                    "dots",
                    "validation",
                    "fallback",
                ])
                .output(output()),
        )
}

fn validate_command() -> CommandSpec {
    CommandSpec::new(["validate"])
        .about("Validate an existing value as a path segment.")
        .arg(ArgSpec::positional("value", 0, "VALUE").about("Value to validate as a path segment"))
        .arg(
            ArgSpec::option_enum("--policy", ["local-path", "url-path"])
                .value_name("POLICY")
                .default("local-path")
                .about("Path-segment kind to validate against"),
        )
        .combination(
            Combination::new("validate")
                .action("validate")
                .required(["value"])
                .optional(["policy"])
                .output(output()),
        )
}

/// One skill verb, as two shapes.
///
/// `--skills-dir` names a single directory, so it is meaningless when the verb
/// fans out across every agent. Registering that as two shapes rather than one
/// shape plus a runtime check means the illegal mix is rejected by the parser,
/// and both legal mixes are visible in one `--help`.
fn skill_command(verb: &str, about: &str, force: bool) -> CommandSpec {
    let mut command = CommandSpec::new(["skill", verb])
        .about(about)
        .arg(
            ArgSpec::option_enum("--agent", std::iter::once(EVERY_AGENT).chain(AGENTS))
                .value_name("AGENT")
                .default(EVERY_AGENT)
                .about("Agent to manage"),
        )
        .arg(
            ArgSpec::option_enum("--scope", ["personal", "workspace"])
                .value_name("SCOPE")
                .default("personal")
                .about("Skill scope"),
        )
        .arg(ArgSpec::option("--skills-dir", "DIR").about("Directory that contains skill folders"));

    let mut every: Vec<&str> = vec!["scope"];
    let mut named: Vec<&str> = vec!["scope", "skills_dir"];
    if force {
        command =
            command.arg(ArgSpec::flag("--force").about(
                "Overwrite or remove an unmanaged Agent-First Slug skill at the target path",
            ));
        every.push("force");
        named.push("force");
    }

    command
        .combination(
            Combination::new(format!("skill-{verb}-every-agent"))
                .action(format!("skill_{verb}"))
                .about("Target every agent that supports the scope")
                .fixed("agent", EVERY_AGENT)
                .optional(every)
                .output(output()),
        )
        .combination(
            Combination::new(format!("skill-{verb}-one-agent"))
                .action(format!("skill_{verb}"))
                .about("Target one named agent; only this shape accepts --skills-dir")
                .fixed_one_of("agent", AGENTS)
                .optional(named)
                .output(output()),
        )
}

fn main() -> ExitCode {
    let cli = match cli_spec() {
        Ok(cli) => cli,
        Err(error) => return emit_startup_error("cli_spec_invalid", &error.to_string()),
    };
    let app = match cli.bind_actions([
        (
            "slugify",
            run_slugify as fn(&ResolvedInvocation) -> ExitCode,
        ),
        ("validate", run_validate),
        ("skill_status", run_skill_status),
        ("skill_install", run_skill_install),
        ("skill_uninstall", run_skill_uninstall),
    ]) {
        Ok(app) => app,
        Err(error) => return emit_startup_error("cli_actions_invalid", &error.to_string()),
    };

    let outcome = match app.resolve_from(std::env::args_os()) {
        Ok(outcome) => outcome,
        // Rejected before anything ran: the error names its own rule in
        // `error.code`, and always lands on the diagnostic stream.
        Err(error) => {
            return emit_event(
                cli_error_event(&error),
                OutputFormat::Json,
                OutputTo::Stderr,
                error.exit_code(),
            );
        }
    };

    match outcome {
        CliOutcome::Run(invocation) => app.execute(&invocation),
        // `--docs` renders the whole registry as raw Markdown, so it carries no
        // format of its own and never becomes a protocol event.
        CliOutcome::Docs(docs) => write_text(
            &render_cli_reference(&cli),
            stream_of(docs.output_plan(), false),
        ),
        CliOutcome::Help(help) => {
            let (format, output_to) = plan_output(help.output_plan());
            if format == OutputFormat::Plain {
                write_text(&help.plain(), stream_of(help.output_plan(), false))
            } else {
                emit_event(cli_help_event(&help), format, output_to, 0)
            }
        }
        CliOutcome::Version(version) => {
            let (format, output_to) = plan_output(version.output_plan());
            emit_event(cli_version_event(&version), format, output_to, 0)
        }
    }
}

fn plan_output(plan: &OutputPlan) -> (OutputFormat, OutputTo) {
    let format = plan
        .format()
        .and_then(|format| cli_parse_output(format).ok())
        .unwrap_or(OutputFormat::Json);
    let output_to = plan
        .destination()
        .and_then(|destination| OutputTo::parse(destination).ok())
        .unwrap_or(OutputTo::Split);
    (format, output_to)
}

fn stream_of(plan: &OutputPlan, is_error: bool) -> OutputTo {
    if is_error || plan.destination() == Some("stderr") {
        OutputTo::Stderr
    } else {
        OutputTo::Stdout
    }
}

fn invocation_string(invocation: &ResolvedInvocation, id: &str) -> String {
    invocation
        .required(id)
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn invocation_optional_string(invocation: &ResolvedInvocation, id: &str) -> Option<String> {
    invocation
        .optional(id)
        .and_then(CliValue::as_str)
        .map(str::to_string)
}

fn invocation_flag(invocation: &ResolvedInvocation, id: &str) -> bool {
    invocation
        .optional(id)
        .and_then(CliValue::as_bool)
        .unwrap_or(false)
}

fn run_slugify(invocation: &ResolvedInvocation) -> ExitCode {
    let (format, output_to) = plan_output(invocation.output_plan());

    // The registry has no single-character type, so this is the one argument
    // whose shape the parser cannot decide. It reports the same classification
    // the parser would — an invalid argument value, rejected before anything
    // ran — rather than inventing a second spelling for the same failure.
    let raw_delimiter = invocation_optional_string(invocation, "delimiter").unwrap_or_default();
    let mut characters = raw_delimiter.chars();
    let (Some(delimiter), None) = (characters.next(), characters.next()) else {
        return emit_error_with_hint(
            "cli_invalid_argument_value",
            "--delimiter must be exactly one character",
            "pass a single character, for example --delimiter -",
            format,
            output_to,
            2,
        );
    };

    let max_slug_chars = match invocation.optional("max_chars").and_then(CliValue::as_i64) {
        None => None,
        Some(value) if value >= 0 => Some(value as usize),
        Some(_) => {
            return emit_error_with_hint(
                "cli_invalid_argument_value",
                "--max-chars must not be negative",
                "pass zero or a positive count",
                format,
                output_to,
                2,
            );
        }
    };

    let config = SlugConfig {
        replacement_delimiter: delimiter,
        lowercase_enabled: !invocation_flag(invocation, "no_lowercase"),
        max_slug_chars,
        allowed_character_set: charset_of(invocation),
        dot_handling_policy: dots_of(invocation),
        transliteration_policy: TransliterationPolicy::None,
        validation_policy: policy_of(invocation, "validation"),
        empty_output_policy: match invocation_optional_string(invocation, "fallback") {
            Some(fallback) => EmptyOutputPolicy::UseFallbackSlug(fallback),
            None => EmptyOutputPolicy::KeepEmptySlug,
        },
    };

    match slugify(&invocation_string(invocation, "input"), &config) {
        Ok(result) => emit_slug_result(&result, format, output_to),
        Err(error) => emit_error("slug_error", &error.to_string(), format, output_to, 1),
    }
}

fn run_validate(invocation: &ResolvedInvocation) -> ExitCode {
    let (format, output_to) = plan_output(invocation.output_plan());
    let value = invocation_string(invocation, "value");
    match validate_slug(&value, policy_of(invocation, "policy")) {
        Ok(()) => emit_result(
            json!({ "code": "validate", "value": value, "valid": true }),
            format,
            output_to,
        ),
        Err(error) => emit_error("slug_error", &error.to_string(), format, output_to, 1),
    }
}

fn charset_of(invocation: &ResolvedInvocation) -> AllowedCharacterSet {
    match invocation_optional_string(invocation, "charset").as_deref() {
        Some("ascii-alphanumeric") => AllowedCharacterSet::AsciiAlphanumericCharacters,
        Some("unicode-letters-digits") => AllowedCharacterSet::UnicodeLettersAndDecimalDigits,
        _ => AllowedCharacterSet::UnicodeAlphanumericCharacters,
    }
}

fn dots_of(invocation: &ResolvedInvocation) -> DotHandlingPolicy {
    match invocation_optional_string(invocation, "dots").as_deref() {
        Some("preserve") => DotHandlingPolicy::PreserveAllDots,
        Some("preserve-between-digits") => DotHandlingPolicy::PreserveDotsBetweenDecimalDigits,
        _ => DotHandlingPolicy::ReplaceAllDots,
    }
}

fn policy_of(invocation: &ResolvedInvocation, id: &str) -> SlugValidationPolicy {
    match invocation_optional_string(invocation, id).as_deref() {
        Some("local-path") => SlugValidationPolicy::LocalPathSegment,
        Some("url-path") => SlugValidationPolicy::UrlPathSegment,
        _ => SlugValidationPolicy::None,
    }
}

fn run_skill_status(invocation: &ResolvedInvocation) -> ExitCode {
    run_skill(invocation, SkillAction::Status)
}

fn run_skill_install(invocation: &ResolvedInvocation) -> ExitCode {
    run_skill(invocation, SkillAction::Install)
}

fn run_skill_uninstall(invocation: &ResolvedInvocation) -> ExitCode {
    run_skill(invocation, SkillAction::Uninstall)
}

fn run_skill(invocation: &ResolvedInvocation, action: SkillAction) -> ExitCode {
    let (format, output_to) = plan_output(invocation.output_plan());
    let options = SkillOptions {
        agent: match invocation_optional_string(invocation, "agent").as_deref() {
            Some("codex") => SkillAgentSelection::Codex,
            Some("claude-code") => SkillAgentSelection::ClaudeCode,
            Some("opencode") => SkillAgentSelection::Opencode,
            Some("hermes") => SkillAgentSelection::Hermes,
            _ => SkillAgentSelection::All,
        },
        scope: match invocation_optional_string(invocation, "scope").as_deref() {
            Some("workspace") => SkillScope::Workspace,
            _ => SkillScope::Personal,
        },
        skills_dir: invocation_optional_string(invocation, "skills_dir"),
        force: invocation_flag(invocation, "force"),
    };

    match skill::run_skill_admin(&SKILL_SPEC, action, &options) {
        Ok(report) => match serde_json::to_value(&report) {
            Ok(value) => emit_result(value, format, output_to),
            Err(error) => emit_error(
                "serialization_failed",
                &format!("failed to serialize skill report: {error}"),
                format,
                output_to,
                1,
            ),
        },
        Err(err) => {
            let event = agent_first_data::json_error("skill_error", &err.message)
                .hint_if_some(err.hint.as_deref())
                .field(
                    "partial_report",
                    err.partial_report
                        .and_then(|report| serde_json::to_value(report).ok())
                        .unwrap_or(Value::Null),
                )
                .build();
            match event {
                Ok(event) => emit_event(event, format, output_to, 1),
                Err(_) => ExitCode::from(4),
            }
        }
    }
}

fn emit_slug_result(result: &SlugResult, format: OutputFormat, output_to: OutputTo) -> ExitCode {
    emit_result(
        json!({
            "code": "slugify",
            "slug": result.slug,
            "changed_from_input": result.changed_from_input,
        }),
        format,
        output_to,
    )
}

fn emit_result(value: Value, format: OutputFormat, output_to: OutputTo) -> ExitCode {
    let mut emitter = CliEmitter::from_output_to(output_to, format).with_strict_protocol();
    match emitter.emit_result(value) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(4),
    }
}

fn emit_error(
    code: &str,
    message: &str,
    format: OutputFormat,
    output_to: OutputTo,
    exit_code: u8,
) -> ExitCode {
    let mut emitter = CliEmitter::from_output_to(output_to, format).with_strict_protocol();
    match emitter.emit_error(code, message) {
        Ok(()) => ExitCode::from(exit_code),
        Err(_) => ExitCode::from(4),
    }
}

fn emit_error_with_hint(
    code: &str,
    message: &str,
    hint: &str,
    format: OutputFormat,
    output_to: OutputTo,
    exit_code: u8,
) -> ExitCode {
    match agent_first_data::json_error(code, message)
        .hint(hint)
        .build()
    {
        Ok(event) => emit_event(event, format, output_to, exit_code),
        Err(_) => ExitCode::from(4),
    }
}

fn emit_event(
    event: agent_first_data::Event,
    format: OutputFormat,
    output_to: OutputTo,
    exit_code: u8,
) -> ExitCode {
    let mut emitter = CliEmitter::from_output_to(output_to, format).with_strict_protocol();
    match emitter.emit(event) {
        Ok(()) => ExitCode::from(exit_code),
        Err(_) => ExitCode::from(4),
    }
}

/// A registry that fails to build is a programming error in this binary, not a
/// caller mistake, so it reports before any output contract has been resolved.
fn emit_startup_error(code: &str, message: &str) -> ExitCode {
    emit_error(code, message, OutputFormat::Json, OutputTo::Stderr, 1)
}

// AFDATA injects the raw outcomes this writes (`--docs`, plain help), so it owns
// the routing and the rule that a closed reader is success rather than failure.
fn write_text(text: &str, output_to: OutputTo) -> ExitCode {
    match agent_first_data::write_raw(text, output_to) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(4),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_builds_and_every_shape_is_reachable() {
        let cli = match cli_spec() {
            Ok(cli) => cli,
            Err(error) => panic!("registry must build: {error}"),
        };
        // Each generated argv must resolve back to the shape it came from, so
        // an overlapping or unreachable combination fails here rather than at a
        // caller's first invocation.
        let synthetics = cli.synthetic_invocations();
        assert!(!synthetics.is_empty(), "the registry generated no fixtures");
        for synthetic in synthetics {
            let argv = synthetic.argv.clone();
            match cli.resolve_from(argv.clone()) {
                Ok(CliOutcome::Run(invocation)) => assert_eq!(
                    invocation.combination_id(),
                    synthetic.combination_id,
                    "{argv:?} resolved to the wrong shape"
                ),
                Ok(_) => panic!("{argv:?} did not resolve to a run"),
                Err(error) => panic!("{argv:?} failed to resolve: {}", error.message),
            }
        }
    }

    #[test]
    fn skills_dir_requires_one_named_agent() {
        let cli = match cli_spec() {
            Ok(cli) => cli,
            Err(error) => panic!("registry must build: {error}"),
        };
        let error =
            match cli.resolve_from(["afslug", "skill", "install", "--skills-dir", "/tmp/skills"]) {
                Err(error) => error,
                Ok(_) => panic!("--skills-dir without an explicit --agent must be rejected"),
            };
        assert_eq!(
            error.rule,
            agent_first_data::CliErrorRule::UnregisteredCombination
        );
    }
}
