use std::io;
use std::process::ExitCode;

use agent_first_data::skill::{
    self, SkillAction, SkillAgentSelection, SkillAsset, SkillOptions, SkillScope, SkillSpec,
};
use agent_first_data::{CliEmitter, OutputFormat, cli_parse_output};
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
    disable_version_flag = true
)]
struct Args {
    #[command(subcommand)]
    command: Command,

    /// Output format: json, yaml, or plain
    #[arg(long, global = true, default_value = "json")]
    output: String,

    /// Print the CLI version
    #[arg(short = 'V', long, action = ArgAction::SetTrue)]
    version: bool,
}

#[derive(Subcommand)]
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
    let build = match env!("GIT_SHA") {
        "unknown" => None,
        sha => Some(sha),
    };
    match agent_first_data::cli_handle_version_or_continue(
        &raw_args,
        &Args::command(),
        "afslug",
        Some(env!("DISPLAY_NAME")),
        env!("CARGO_PKG_VERSION"),
        build,
    ) {
        Ok(Some(version)) => return write_text(&version),
        Ok(None) => {}
        Err(event) => return emit_event_error(event, OutputFormat::Json, 2),
    }

    // Render `--help` (and `--help --recursive --output markdown`, the form the
    // release pipeline exports into docs/cli.md) through afdata's help renderer
    // before clap parses, so afslug's CLI docs match every other spore's format.
    match agent_first_data::cli_handle_help_or_continue(
        &raw_args,
        &Args::command(),
        &agent_first_data::HelpConfig::human_cli_default(),
    ) {
        Ok(Some(help)) => return write_text(&help),
        Ok(None) => {}
        Err(error) => return emit_value_error(error, OutputFormat::Json, 2),
    }

    let args = match Args::try_parse() {
        Ok(args) => args,
        Err(error) if error.kind() == ErrorKind::DisplayHelp => {
            return write_text(&error.render().to_string());
        }
        Err(error) => return emit_error("cli_error", &error.to_string(), OutputFormat::Json, 2),
    };
    let _ = args.version;

    let output = match cli_parse_output(&args.output) {
        Ok(output) => output,
        Err(message) => return emit_error("cli_error", &message, OutputFormat::Json, 2),
    };

    match args.command {
        Command::Slugify(slugify_args) => run_slugify(slugify_args, output),
        Command::Validate(validate_args) => run_validate(validate_args, output),
        Command::Skill(skill_cmd) => run_skill(skill_cmd, output),
    }
}

fn run_skill(cmd: SkillCommand, output: OutputFormat) -> ExitCode {
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
                    CliEmitter::new(io::stdout().lock(), output).with_strict_protocol();
                match emitter.emit_result(value) {
                    Ok(()) => ExitCode::SUCCESS,
                    Err(_) => ExitCode::from(4),
                }
            }
            Err(error) => emit_error(
                "serialization_failed",
                &format!("failed to serialize skill report: {error}"),
                output,
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
                Ok(event) => emit_event_error(event, output, 1),
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

fn run_slugify(args: SlugifyArgs, output: OutputFormat) -> ExitCode {
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
        Ok(result) => emit_slug_result(&result, output),
        Err(error) => emit_error("slug_error", &error.to_string(), output, 1),
    }
}

fn run_validate(args: ValidateArgs, output: OutputFormat) -> ExitCode {
    match validate_slug(&args.value, args.policy.into_lib()) {
        Ok(()) => {
            let mut emitter = CliEmitter::new(io::stdout().lock(), output).with_strict_protocol();
            match emitter.emit_result(json!({
                "code": "validate",
                "value": args.value,
                "valid": true,
            })) {
                Ok(()) => ExitCode::SUCCESS,
                Err(_) => ExitCode::from(4),
            }
        }
        Err(error) => emit_error("slug_error", &error.to_string(), output, 1),
    }
}

fn emit_slug_result(result: &SlugResult, output: OutputFormat) -> ExitCode {
    let mut emitter = CliEmitter::new(io::stdout().lock(), output).with_strict_protocol();
    match emitter.emit_result(json!({
        "code": "slugify",
        "slug": result.slug,
        "changed_from_input": result.changed_from_input,
    })) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(4),
    }
}

fn emit_error(code: &str, message: &str, output: OutputFormat, exit_code: u8) -> ExitCode {
    let mut emitter = CliEmitter::new(io::stdout().lock(), output).with_strict_protocol();
    match emitter.emit_error(code, message) {
        Ok(()) => ExitCode::from(exit_code),
        Err(_) => ExitCode::from(4),
    }
}

fn emit_event_error(
    event: agent_first_data::Event,
    output: OutputFormat,
    exit_code: u8,
) -> ExitCode {
    let mut emitter = CliEmitter::new(io::stdout().lock(), output).with_strict_protocol();
    match emitter.emit(event) {
        Ok(()) => ExitCode::from(exit_code),
        Err(_) => ExitCode::from(4),
    }
}

fn emit_value_error(error: Value, output: OutputFormat, exit_code: u8) -> ExitCode {
    let mut emitter = CliEmitter::new(io::stdout().lock(), output).with_strict_protocol();
    match emitter.emit_validated_value(error) {
        Ok(()) => ExitCode::from(exit_code),
        Err(_) => ExitCode::from(4),
    }
}

fn write_text(text: &str) -> ExitCode {
    use std::io::Write;

    match io::stdout().lock().write_all(text.as_bytes()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(4),
    }
}
