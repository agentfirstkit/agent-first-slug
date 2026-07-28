use std::io::{self, Write};
use std::process::ExitCode;

use agent_first_data::skill::{
    self, SkillAction, SkillAgentSelection, SkillAsset, SkillOptions, SkillScope, SkillSpec,
};
use agent_first_data::{CliEmitter, OutputFormat, OutputTo, cli_parse_output};
use agent_first_slug::{
    AllowedCharacterSet, DotHandlingPolicy, EmptyOutputPolicy, SlugConfig, SlugResult,
    SlugValidationPolicy, TransliterationPolicy, slugify, validate_slug,
};
use clap::{ArgAction, CommandFactory, Parser, Subcommand, ValueEnum, error::ErrorKind};
use serde_json::{Value, json};

#[derive(Parser)]
#[command(
    name = "afslug",
    about = "Generate and validate slugs with explicit agent-first-slug rules.",
    disable_version_flag = true,
    disable_help_subcommand = true
)]
struct Args {
    #[command(subcommand)]
    command: Command,

    /// Output format: json, yaml, or plain
    #[arg(long, global = true, default_value = "json")]
    output: String,

    /// Output routing: split, stdout, or stderr
    #[arg(long, global = true, default_value = "split")]
    output_to: String,

    /// Print the CLI version
    #[arg(long, action = ArgAction::SetTrue)]
    version: bool,
}

#[derive(Subcommand)]
#[command(disable_help_subcommand = true)]
enum Command {
    /// Generate a slug from input text.
    Slugify(SlugifyArgs),
    /// Validate an existing value as a path segment.
    Validate(ValidateArgs),
    /// Manage Agent-First Slug skills for Codex, Claude Code, opencode, and Hermes.
    Skill(SkillCommand),
}

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

#[derive(clap::Args)]
struct SkillCommand {
    #[command(subcommand)]
    action: SkillCliAction,
}

#[derive(Subcommand)]
#[command(disable_help_subcommand = true)]
enum SkillCliAction {
    /// Show whether the Agent-First Slug skill is installed, valid, and up to date.
    Status(SkillTargetArgs),
    /// Install the Agent-First Slug skill.
    Install(SkillWriteArgs),
    /// Remove an afslug-managed Agent-First Slug skill.
    Uninstall(SkillWriteArgs),
}

#[derive(clap::Args)]
struct SkillTargetArgs {
    /// Agent to manage. Defaults to all personal skill targets.
    #[arg(long = "agent", value_enum, default_value_t = SkillAgentArg::All)]
    agent: SkillAgentArg,
    /// Skill scope.
    #[arg(long = "scope", value_enum, default_value_t = SkillScopeArg::Personal)]
    scope: SkillScopeArg,
    /// Directory that contains skill folders. Requires an explicit single --agent.
    #[arg(long = "skills-dir")]
    skills_dir: Option<String>,
}

#[derive(clap::Args)]
struct SkillWriteArgs {
    #[command(flatten)]
    target: SkillTargetArgs,
    /// Overwrite or remove an unmanaged Agent-First Slug skill at the target path.
    #[arg(long)]
    force: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum SkillAgentArg {
    /// Manage every agent that supports the requested scope.
    All,
    /// Codex under $CODEX_HOME/skills.
    Codex,
    /// Claude Code under ~/.claude/skills or .claude/skills.
    #[value(name = "claude-code", alias = "claude")]
    ClaudeCode,
    /// opencode under ~/.config/opencode/skills or .opencode/skills.
    Opencode,
    /// Hermes under $HERMES_HOME/skills or ~/.hermes/skills.
    Hermes,
}

#[derive(Clone, Copy, ValueEnum)]
enum SkillScopeArg {
    /// Install under the user-level skills directory.
    Personal,
    /// Install under the current workspace's skills directory.
    Workspace,
}

#[derive(clap::Args)]
struct SlugifyArgs {
    /// Text to slugify
    input: String,

    /// Delimiter inserted for each run of filtered characters
    #[arg(long, default_value_t = '-')]
    delimiter: char,

    /// Keep the original case instead of lowercasing the slug
    #[arg(long, action = ArgAction::SetTrue)]
    no_lowercase: bool,

    /// Cap the slug to at most N Unicode characters
    #[arg(long, value_name = "N")]
    max_chars: Option<usize>,

    /// Character set kept from the input after filtering
    #[arg(long, default_value = "unicode-alphanumeric")]
    charset: CharsetArg,

    /// How input dots are handled before other characters become delimiters
    #[arg(long, default_value = "replace")]
    dots: DotsArg,

    /// Validation applied to the generated slug
    #[arg(long, default_value = "none")]
    validation: ValidationArg,

    /// Slug substituted when the generated slug would otherwise be empty
    #[arg(long, value_name = "SLUG")]
    fallback: Option<String>,
}

#[derive(clap::Args)]
struct ValidateArgs {
    /// Value to validate as a path segment
    value: String,

    /// Path-segment kind to validate against
    #[arg(long, default_value = "local-path")]
    policy: PolicyArg,
}

#[derive(Clone, Copy, ValueEnum)]
enum CharsetArg {
    /// Unicode alphanumeric characters
    UnicodeAlphanumeric,
    /// ASCII letters and digits only
    AsciiAlphanumeric,
    /// Unicode letters plus decimal digits
    UnicodeLettersDigits,
}

#[derive(Clone, Copy, ValueEnum)]
enum DotsArg {
    /// Treat every dot as a delimiter
    Replace,
    /// Preserve every dot
    Preserve,
    /// Preserve a dot only between two decimal digits
    PreserveBetweenDigits,
}

#[derive(Clone, Copy, ValueEnum)]
enum ValidationArg {
    /// No validation
    None,
    /// Validate as one local filesystem path segment
    LocalPath,
    /// Validate as one URL path segment
    UrlPath,
}

#[derive(Clone, Copy, ValueEnum)]
enum PolicyArg {
    /// One local filesystem path segment
    LocalPath,
    /// One URL path segment
    UrlPath,
}

impl CharsetArg {
    fn into_lib(self) -> AllowedCharacterSet {
        match self {
            Self::UnicodeAlphanumeric => AllowedCharacterSet::UnicodeAlphanumericCharacters,
            Self::AsciiAlphanumeric => AllowedCharacterSet::AsciiAlphanumericCharacters,
            Self::UnicodeLettersDigits => AllowedCharacterSet::UnicodeLettersAndDecimalDigits,
        }
    }
}

impl DotsArg {
    fn into_lib(self) -> DotHandlingPolicy {
        match self {
            Self::Replace => DotHandlingPolicy::ReplaceAllDots,
            Self::Preserve => DotHandlingPolicy::PreserveAllDots,
            Self::PreserveBetweenDigits => DotHandlingPolicy::PreserveDotsBetweenDecimalDigits,
        }
    }
}

impl ValidationArg {
    fn into_lib(self) -> SlugValidationPolicy {
        match self {
            Self::None => SlugValidationPolicy::None,
            Self::LocalPath => SlugValidationPolicy::LocalPathSegment,
            Self::UrlPath => SlugValidationPolicy::UrlPathSegment,
        }
    }
}

impl PolicyArg {
    fn into_lib(self) -> SlugValidationPolicy {
        match self {
            Self::LocalPath => SlugValidationPolicy::LocalPathSegment,
            Self::UrlPath => SlugValidationPolicy::UrlPathSegment,
        }
    }
}

fn main() -> ExitCode {
    let raw_args = std::env::args().collect::<Vec<_>>();
    let output_to = match requested_output_to(&raw_args) {
        Ok(output_to) => output_to,
        Err(message) => {
            return emit_error(
                "cli_error",
                &message,
                OutputFormat::Json,
                OutputTo::Split,
                2,
            );
        }
    };
    let build = match env!("GIT_SHA") {
        "unknown" => None,
        sha => Some(sha),
    };
    // Resolve version and progressively scoped help before clap so every
    // machine-facing discovery path follows the same AFDATA output contract.
    match agent_first_data::cli_handle_version_or_help_or_continue(
        &raw_args,
        &Args::command(),
        &agent_first_data::HelpConfig::output_aware(),
        "afslug",
        Some(env!("DISPLAY_NAME")),
        env!("CARGO_PKG_VERSION"),
        build,
    ) {
        Ok(Some(output)) => return write_text(&output, output_to),
        Ok(None) => {}
        Err(error) => return emit_value_error(error, OutputFormat::Json, output_to, 2),
    }

    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) if error.kind() == ErrorKind::DisplayHelp => {
            return write_text(&error.render().to_string(), output_to);
        }
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand | ErrorKind::MissingSubcommand
            ) =>
        {
            return emit_event_error(
                agent_first_data::build_cli_error(
                    "a command is required",
                    Some("try: afslug --help"),
                ),
                OutputFormat::Json,
                output_to,
                2,
            );
        }
        Err(error) => {
            return emit_error(
                "cli_error",
                &error.to_string(),
                OutputFormat::Json,
                output_to,
                2,
            );
        }
    };
    let _ = args.version;
    debug_assert_eq!(OutputTo::parse(&args.output_to).ok(), Some(output_to));

    let output = match cli_parse_output(&args.output) {
        Ok(output) => output,
        Err(message) => {
            return emit_error("cli_error", &message, OutputFormat::Json, output_to, 2);
        }
    };

    match args.command {
        Command::Slugify(slugify_args) => run_slugify(slugify_args, output, output_to),
        Command::Validate(validate_args) => run_validate(validate_args, output, output_to),
        Command::Skill(skill_cmd) => run_skill(skill_cmd, output, output_to),
    }
}

fn requested_output_to(raw_args: &[String]) -> Result<OutputTo, String> {
    let mut output_to = OutputTo::Split;
    let mut args = raw_args.iter().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--" {
            break;
        }
        if let Some(value) = arg.strip_prefix("--output-to=") {
            output_to = OutputTo::parse(value)?;
        } else if arg == "--output-to" {
            let value = args.next().ok_or_else(|| {
                "--output-to requires a value: expected split, stdout, or stderr".to_string()
            })?;
            output_to = OutputTo::parse(value)?;
        }
    }
    Ok(output_to)
}

fn run_skill(cmd: SkillCommand, output: OutputFormat, output_to: OutputTo) -> ExitCode {
    let (action, options) = match cmd.action {
        SkillCliAction::Status(target) => (SkillAction::Status, skill_options(target, false)),
        SkillCliAction::Install(write) => (
            SkillAction::Install,
            skill_options(write.target, write.force),
        ),
        SkillCliAction::Uninstall(write) => (
            SkillAction::Uninstall,
            skill_options(write.target, write.force),
        ),
    };
    match skill::run_skill_admin(&SKILL_SPEC, action, &options) {
        Ok(report) => match serde_json::to_value(&report) {
            Ok(value) => {
                let mut emitter =
                    CliEmitter::from_output_to(output_to, output).with_strict_protocol();
                match emitter.emit_result(value) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(_) => ExitCode::from(4),
                }
            }
            Err(error) => emit_error(
                "serialization_failed",
                &format!("failed to serialize skill report: {error}"),
                output,
                output_to,
                1,
            ),
        },
        Err(err) => {
            let event = agent_first_data::json_error("cli_error", &err.message)
                .hint_if_some(err.hint.as_deref())
                .field(
                    "partial_report",
                    err.partial_report
                        .and_then(|report| serde_json::to_value(report).ok())
                        .unwrap_or(Value::Null),
                )
                .build();
            match event {
                Ok(event) => emit_event_error(event, output, output_to, 1),
                Err(_) => ExitCode::from(4),
            }
        }
    }
}

fn skill_options(target: SkillTargetArgs, force: bool) -> SkillOptions {
    SkillOptions {
        agent: match target.agent {
            SkillAgentArg::All => SkillAgentSelection::All,
            SkillAgentArg::Codex => SkillAgentSelection::Codex,
            SkillAgentArg::ClaudeCode => SkillAgentSelection::ClaudeCode,
            SkillAgentArg::Opencode => SkillAgentSelection::Opencode,
            SkillAgentArg::Hermes => SkillAgentSelection::Hermes,
        },
        scope: match target.scope {
            SkillScopeArg::Personal => SkillScope::Personal,
            SkillScopeArg::Workspace => SkillScope::Workspace,
        },
        skills_dir: target.skills_dir,
        force,
    }
}

fn run_slugify(args: SlugifyArgs, output: OutputFormat, output_to: OutputTo) -> ExitCode {
    // Transliteration is intentionally absent: its policy carries a `'static`
    // replacement map that a CLI cannot build from runtime input, so callers who
    // need it reach for the library.
    let config = SlugConfig {
        replacement_delimiter: args.delimiter,
        lowercase_enabled: !args.no_lowercase,
        max_slug_chars: args.max_chars,
        allowed_character_set: args.charset.into_lib(),
        dot_handling_policy: args.dots.into_lib(),
        transliteration_policy: TransliterationPolicy::None,
        validation_policy: args.validation.into_lib(),
        empty_output_policy: match args.fallback {
            Some(fallback) => EmptyOutputPolicy::UseFallbackSlug(fallback),
            None => EmptyOutputPolicy::KeepEmptySlug,
        },
    };

    match slugify(&args.input, &config) {
        Ok(result) => emit_slug_result(&result, output, output_to),
        Err(error) => emit_error("slug_error", &error.to_string(), output, output_to, 1),
    }
}

fn run_validate(args: ValidateArgs, output: OutputFormat, output_to: OutputTo) -> ExitCode {
    match validate_slug(&args.value, args.policy.into_lib()) {
        Ok(()) => {
            let mut emitter = CliEmitter::from_output_to(output_to, output).with_strict_protocol();
            match emitter.emit_result(json!({
                "code": "validate",
                "value": args.value,
                "valid": true,
            })) {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::from(4),
            }
        }
        Err(error) => emit_error("slug_error", &error.to_string(), output, output_to, 1),
    }
}

fn emit_slug_result(result: &SlugResult, output: OutputFormat, output_to: OutputTo) -> ExitCode {
    let mut emitter = CliEmitter::from_output_to(output_to, output).with_strict_protocol();
    match emitter.emit_result(json!({
        "code": "slugify",
        "slug": result.slug,
        "changed_from_input": result.changed_from_input,
    })) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(4),
    }
}

fn emit_error(
    code: &str,
    message: &str,
    output: OutputFormat,
    output_to: OutputTo,
    exit_code: u8,
) -> ExitCode {
    let mut emitter = CliEmitter::from_output_to(output_to, output).with_strict_protocol();
    match emitter.emit_error(code, message) {
        Ok(()) => ExitCode::from(exit_code),
        Err(_) => ExitCode::from(4),
    }
}

fn emit_event_error(
    event: agent_first_data::Event,
    output: OutputFormat,
    output_to: OutputTo,
    exit_code: u8,
) -> ExitCode {
    let mut emitter = CliEmitter::from_output_to(output_to, output).with_strict_protocol();
    match emitter.emit(event) {
        Ok(()) => ExitCode::from(exit_code),
        Err(_) => ExitCode::from(4),
    }
}

fn emit_value_error(
    error: Value,
    output: OutputFormat,
    output_to: OutputTo,
    exit_code: u8,
) -> ExitCode {
    let mut emitter = CliEmitter::from_output_to(output_to, output).with_strict_protocol();
    match emitter.emit_validated_value(error) {
        Ok(()) => ExitCode::from(exit_code),
        Err(_) => ExitCode::from(4),
    }
}

#[allow(clippy::disallowed_methods)]
fn write_text(text: &str, output_to: OutputTo) -> ExitCode {
    let result = match output_to {
        OutputTo::Stderr => io::stderr().lock().write_all(text.as_bytes()),
        OutputTo::Split | OutputTo::Stdout => io::stdout().lock().write_all(text.as_bytes()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(4),
    }
}
