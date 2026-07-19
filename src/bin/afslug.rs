use std::io;
use std::process::ExitCode;

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
    match agent_first_data::cli_handle_version_or_continue(
        &raw_args,
        "afslug",
        env!("CARGO_PKG_VERSION"),
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
