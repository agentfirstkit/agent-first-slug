#![cfg(feature = "cli")]
#![allow(clippy::expect_used)]

use std::process::{Command, Output};

use serde_json::{Value, json};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_afslug"))
        .args(args)
        .output()
        .expect("afslug should run")
}

fn stdout_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).expect("stdout should contain one JSON event")
}

#[test]
fn slugifies_with_a_strict_afdata_result() {
    let output = run(&["slugify", "Hello, 世界!"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        stdout_json(&output),
        json!({
            "kind": "result",
            "result": {
                "code": "slugify",
                "slug": "hello-世界",
                "changed_from_input": true
            },
            "trace": {}
        })
    );
}

#[test]
fn supports_plain_afdata_output() {
    let output = run(&["slugify", "Already-Slug", "--output", "plain"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert_eq!(
        stdout,
        "kind=result result.changed_from_input=true result.code=slugify result.slug=already-slug\n"
    );
}

#[test]
fn supports_yaml_afdata_output() {
    let output = run(&["slugify", "Hello, World!", "--output", "yaml"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("stdout should be UTF-8");
    assert_eq!(
        stdout,
        concat!(
            "---\n",
            "kind: \"result\"\n",
            "result:\n",
            "  changed_from_input: true\n",
            "  code: \"slugify\"\n",
            "  slug: \"hello-world\"\n",
            "trace: {}\n",
        )
    );
}

#[test]
fn slugify_honors_config_flags() {
    // ASCII-only charset drops the CJK run, truncation caps the slug, and the
    // trailing delimiter the cut exposes is stripped.
    let output = run(&[
        "slugify",
        "Rust 版 CLI Tool",
        "--charset",
        "ascii-alphanumeric",
        "--max-chars",
        "8",
    ]);

    assert!(output.status.success());
    assert_eq!(stdout_json(&output)["result"]["slug"], "rust-cli");
}

#[test]
fn slugify_keeps_case_when_lowercasing_is_disabled() {
    let output = run(&["slugify", "Hello World", "--no-lowercase"]);

    assert!(output.status.success());
    assert_eq!(stdout_json(&output)["result"]["slug"], "Hello-World");
}

#[test]
fn slugify_substitutes_fallback_for_empty_output() {
    let output = run(&["slugify", "!!!", "--fallback", "item"]);

    assert!(output.status.success());
    assert_eq!(stdout_json(&output)["result"]["slug"], "item");
}

#[test]
fn slugify_validation_failure_is_a_structured_error() {
    // Punctuation-only input yields an empty slug, which is not a valid URL segment.
    let output = run(&["slugify", "!!!", "--validation", "url-path"]);

    assert_eq!(output.status.code(), Some(1));
    let event = stdout_json(&output);
    assert_eq!(event["kind"], "error");
    assert_eq!(event["error"]["code"], "slug_error");
}

#[test]
fn validate_accepts_a_valid_segment() {
    let output = run(&["validate", "my-slug", "--policy", "url-path"]);

    assert!(output.status.success());
    assert_eq!(
        stdout_json(&output),
        json!({
            "kind": "result",
            "result": {
                "code": "validate",
                "value": "my-slug",
                "valid": true
            },
            "trace": {}
        })
    );
}

#[test]
fn validate_rejects_an_invalid_segment_as_a_structured_error() {
    let output = run(&["validate", "bad/slug", "--policy", "local-path"]);

    assert_eq!(output.status.code(), Some(1));
    let event = stdout_json(&output);
    assert_eq!(event["kind"], "error");
    assert_eq!(event["error"]["code"], "slug_error");
    assert_eq!(event["error"]["retryable"], false);
}

#[test]
fn reports_argument_errors_as_afdata_json() {
    let output = run(&[]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let event = stdout_json(&output);
    assert_eq!(event["kind"], "error");
    assert_eq!(event["error"]["code"], "cli_error");
    assert_eq!(event["error"]["retryable"], false);
    assert_eq!(event["trace"], json!({}));
}

#[test]
fn explicit_json_version_is_structured() {
    let output = run(&["--version", "--output", "json"]);

    assert!(output.status.success());
    assert_eq!(
        stdout_json(&output),
        json!({
            "kind": "result",
            "result": {
                "code": "version",
                "version": env!("CARGO_PKG_VERSION")
            },
            "trace": {}
        })
    );
}
