use std::io;
use std::process::ExitCode;

use agent_first_data::{CliEmitter, OutputFormat, cli_parse_output};
use agent_first_slug::{SlugConfig, slugify};
use clap::{ArgAction, Parser, error::ErrorKind};
use serde_json::json;

#[derive(Parser)]
#[command(
    name = "afslug",
    about = "Generate a slug with the default agent-first-slug rules.",
    disable_version_flag = true
)]
struct Args {
    /// Text to slugify
    input: String,

    /// Output format: json, yaml, or plain
    #[arg(long, global = true, default_value = "json")]
    output: String,

    /// Print the CLI version
    #[arg(short = 'V', long, action = ArgAction::SetTrue)]
    version: bool,
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

    match slugify(&args.input, &SlugConfig::default()) {
        Ok(result) => {
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
        Err(error) => emit_error("slug_error", &error.to_string(), output, 1),
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

fn write_text(text: &str) -> ExitCode {
    use std::io::Write;

    match io::stdout().lock().write_all(text.as_bytes()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(4),
    }
}
