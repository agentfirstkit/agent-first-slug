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
    let output = run(&["Hello, 世界!"]);

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
    let output = run(&["Already-Slug", "--output", "plain"]);

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
    let output = run(&["Hello, World!", "--output", "yaml"]);

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
