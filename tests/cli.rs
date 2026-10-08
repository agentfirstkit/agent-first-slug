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

fn stderr_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stderr).expect("stderr should contain one JSON event")
}

fn help_of(output: &Output) -> Value {
    stdout_json(output)["result"]["help"].clone()
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
    // `trace={}` is present because plain renders an empty container explicitly
    // rather than letting the key vanish; plain is not a lossy view of the JSON.
    assert_eq!(
        stdout,
        "kind=result result.changed_from_input=true result.code=slugify \
         result.slug=already-slug trace={}\n"
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
fn slugify_keeps_combining_marks_whole_only_under_the_marks_charset() {
    for (charset, expected) in [
        ("unicode-alphanumeric", "नमस-ते-ไม-ใช"),
        ("unicode-letters-digits", "नमस-त-ไม-ใช"),
        ("unicode-letters-marks-digits", "नमस्ते-ไม่ใช่"),
    ] {
        let output = run(&["slugify", "नमस्ते ไม่ใช่", "--charset", charset]);
        assert!(output.status.success(), "{charset}");
        assert!(output.stderr.is_empty(), "{charset}");
        assert_eq!(
            stdout_json(&output)["result"]["slug"],
            expected,
            "{charset}"
        );
    }
}

#[test]
fn slugify_holds_a_fallback_to_the_single_delimiter_rule() {
    // A generated slug never repeats its delimiter, so a fallback that does is
    // not one this configuration could have produced.
    let rejected = run(&["slugify", "!!!", "--fallback", "a--b"]);
    assert_eq!(rejected.status.code(), Some(1));
    assert!(rejected.stdout.is_empty());
    let error = stderr_json(&rejected)["error"].clone();
    assert_eq!(error["code"], "slug_error");
    assert_eq!(
        error["message"],
        "fallback slug does not satisfy this configuration: \
         a generated slug never repeats the replacement delimiter"
    );
    assert_eq!(
        error["hint"],
        "pass a --fallback this configuration could itself have produced, \
         or --fallback-verbatim to insert the value as written"
    );

    let verbatim = run(&["slugify", "!!!", "--fallback-verbatim", "a--b"]);
    assert!(verbatim.status.success());
    assert!(verbatim.stderr.is_empty());
    assert_eq!(stdout_json(&verbatim)["result"]["slug"], "a--b");

    let single = run(&["slugify", "!!!", "--fallback", "a-b"]);
    assert!(single.status.success());
    assert!(single.stderr.is_empty());
    assert_eq!(stdout_json(&single)["result"]["slug"], "a-b");
}

#[test]
fn slugify_substitutes_verbatim_fallback_for_empty_output() {
    let output = run(&[
        "slugify",
        "!!!",
        "--charset",
        "ascii-alphanumeric",
        "--max-chars",
        "3",
        "--validation",
        "url-path",
        "--fallback-verbatim",
        "Stored.Name",
    ]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(stdout_json(&output)["result"]["slug"], "Stored.Name");
}

#[test]
fn slugify_rejects_both_fallback_rules_before_execution() {
    for args in [
        vec![
            "slugify",
            "!!!",
            "--fallback",
            "item",
            "--fallback-verbatim",
            "Stored.Name",
            "--output",
            "yaml",
            "--output-to",
            "stdout",
        ],
        vec![
            "slugify",
            "already-slug",
            "--fallback-verbatim",
            "Stored.Name",
            "--fallback",
            "item",
        ],
    ] {
        let output = run(&args);

        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        let event = stderr_json(&output);
        assert_eq!(event["kind"], "error", "{args:?}");
        assert_eq!(
            event["error"]["code"], "cli_unregistered_combination",
            "{args:?}"
        );
        assert_eq!(event["error"]["retryable"], false, "{args:?}");
        assert_eq!(event["trace"], json!({}), "{args:?}");
    }
}

#[test]
fn slugify_invalid_values_ignore_requested_format_and_destination() {
    for args in [
        vec![
            "slugify",
            "hello",
            "--delimiter",
            "xx",
            "--output",
            "yaml",
            "--output-to",
            "stdout",
        ],
        vec![
            "slugify",
            "hello",
            "--max-chars",
            "-1",
            "--output",
            "plain",
            "--output-to",
            "stdout",
        ],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty(), "{args:?}");
        let event = stderr_json(&output);
        assert_eq!(event["kind"], "error", "{args:?}");
        assert_eq!(
            event["error"]["code"], "cli_invalid_argument_value",
            "{args:?}"
        );
        assert_eq!(event["error"]["retryable"], false, "{args:?}");
        assert_eq!(event["trace"], json!({}), "{args:?}");
    }
}

#[test]
fn slugify_validation_failure_is_a_structured_error() {
    // Punctuation-only input yields an empty slug, which is not a valid URL segment.
    let output = run(&["slugify", "!!!", "--validation", "url-path"]);

    assert_eq!(output.status.code(), Some(1));
    let event = stderr_json(&output);
    assert_eq!(event["kind"], "error");
    assert_eq!(event["error"]["code"], "slug_error");
}

#[test]
fn a_remedy_offered_here_is_one_a_command_line_can_type() {
    // The library's own message named `UseVerbatimFallbackSlug`, which is a
    // Rust path and not something an agent holding a command line can type.
    // What is wrong is the library's to say; what to do about it belongs to
    // whichever surface is being spoken to.
    for (args, fragment) in [
        (
            vec![
                "slugify",
                "!!!",
                "--charset",
                "ascii-alphanumeric",
                "--max-chars",
                "8",
                "--fallback",
                "Ünïcode",
            ],
            "--fallback-verbatim",
        ),
        (
            vec!["slugify", "hello world", "--delimiter", "a"],
            "--delimiter",
        ),
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        let error = stderr_json(&output)["error"].clone();
        assert_eq!(error["code"], "slug_error", "{args:?}");

        let hint = error["hint"].as_str().unwrap_or_default();
        assert!(
            hint.contains(fragment),
            "{args:?}: hint must name the flag to reach for, got {hint:?}"
        );

        // A capital immediately after a lowercase letter is how a Rust
        // identifier reads in prose and how nothing else does: English puts a
        // space or a mark before a capital. No message that reaches a command
        // line should contain one.
        let message = error["message"].as_str().expect("a message");
        let identifier = message
            .chars()
            .collect::<Vec<_>>()
            .windows(2)
            .any(|run| run[0].is_ascii_lowercase() && run[1].is_ascii_uppercase());
        assert!(
            !identifier,
            "{args:?}: message reads like a Rust identifier, got {message:?}"
        );
    }
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
    let event = stderr_json(&output);
    assert_eq!(event["kind"], "error");
    assert_eq!(event["error"]["code"], "slug_error");
    assert_eq!(event["error"]["retryable"], false);
}

#[test]
fn reports_argument_errors_as_afdata_json() {
    let output = run(&[]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let event = stderr_json(&output);
    assert_eq!(event["kind"], "error");
    // The classification is the code itself, so an agent branches on one key
    // rather than parsing a message or reading a second field.
    assert_eq!(event["error"]["code"], "cli_unregistered_combination");
    assert_eq!(event["error"]["retryable"], false);
    assert_eq!(event["trace"], json!({}));
}

#[test]
fn explicit_json_version_is_structured() {
    let output = run(&["--version", "--output", "json"]);

    assert!(output.status.success());
    let value = stdout_json(&output);
    assert_eq!(value["kind"], "result");
    assert_eq!(value["result"]["code"], "version");
    assert_eq!(value["result"]["name"], "afslug");
    assert_eq!(value["result"]["display_name"], "Agent-First Slug");
    assert_eq!(value["result"]["version"], env!("CARGO_PKG_VERSION"));
    // "build" (git SHA) is environment-dependent (absent without a reachable
    // .git, e.g. a source tarball) so it is deliberately not asserted here.
    assert_eq!(value["trace"], json!({}));
}

#[test]
fn bare_version_is_structured() {
    let output = run(&["--version"]);

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let value = stdout_json(&output);
    assert_eq!(value["kind"], "result");
    assert_eq!(value["result"]["code"], "version");
    assert_eq!(value["result"]["name"], "afslug");
    assert_eq!(value["result"]["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn short_flags_do_not_exist() {
    // The registry has no short syntax at all, so `-V` is not a rejected alias
    // of `--version` — it is simply not an argument.
    for short in ["-V", "-h"] {
        let output = run(&[short]);

        assert_eq!(output.status.code(), Some(2), "{short} must be rejected");
        assert!(output.stdout.is_empty());
        let value = stderr_json(&output);
        assert_eq!(value["kind"], "error");
        assert_eq!(value["error"]["code"], "cli_unknown_argument");
        // The rejection classifies the token without quoting it back: the
        // message is a fixed string, and `error.code` plus the hint are what
        // the caller acts on.
        assert_eq!(value["error"]["message"], "unknown short argument");
    }
}

#[test]
fn root_help_routes_to_commands_without_listing_their_arguments() {
    let root = run(&["--help"]);
    assert!(root.status.success());
    assert!(root.stderr.is_empty());
    assert_eq!(stdout_json(&root)["result"]["code"], "help");

    let help = help_of(&root);
    assert_eq!(help["schema"], "cli-help-v2");
    assert_eq!(help["command_path"], "afslug");
    // The root registers no combination, so it has no shape of its own — it is
    // a router, and every entry is a ready-to-run next call.
    assert!(help.get("shapes").is_none(), "{help}");
    assert_eq!(
        help["subcommands"],
        json!([
            "afslug skill --help",
            "afslug slugify --help",
            "afslug validate --help"
        ])
    );
    // `--docs` is injected but deliberately invisible: no agent calls it, and
    // it would cost a line of every discovery response.
    assert!(!help.to_string().contains("--docs"), "{help}");
}

#[test]
fn command_help_answers_in_one_round_trip() {
    let scoped = run(&["slugify", "--help"]);
    assert!(scoped.status.success());
    let help = help_of(&scoped);
    assert_eq!(help["command_path"], "afslug slugify");

    let shapes = help["shapes"].as_array().expect("slugify has three shapes");
    assert_eq!(shapes.len(), 3);
    for (id, fallback) in [
        ("slugify", None),
        ("slugify-fallback", Some("--fallback <SLUG>")),
        (
            "slugify-fallback-verbatim",
            Some("--fallback-verbatim <SLUG>"),
        ),
    ] {
        let shape = shapes
            .iter()
            .find(|shape| shape["id"] == id)
            .unwrap_or_else(|| panic!("missing shape {id}: {help}"));
        let usage = shape["usage"].as_str().expect("usage is a string");
        assert!(usage.starts_with("afslug slugify <TEXT>"), "{usage}");
        assert!(
            shape["about"]
                .as_str()
                .is_some_and(|about| !about.is_empty())
        );

        // Each shape is complete, including closed value sets, in this one
        // answer rather than requiring a second discovery call.
        for optional in [
            "[--delimiter <CHAR>]",
            "[--no-lowercase]",
            "[--max-chars <N>]",
            "[--charset <unicode-alphanumeric|ascii-alphanumeric|unicode-letters-digits|unicode-letters-marks-digits>]",
            "[--dots <replace|preserve|preserve-between-digits>]",
            "[--validation <none|local-path|url-path>]",
        ] {
            assert!(usage.contains(optional), "{optional} missing from {usage}");
        }
        for option in ["--fallback <SLUG>", "--fallback-verbatim <SLUG>"] {
            if fallback == Some(option) {
                assert!(usage.contains(option), "{option} missing from {usage}");
                assert!(
                    !usage.contains(&format!("[{option}]")),
                    "{option} must be required in {usage}"
                );
            } else {
                assert!(!usage.contains(option), "{option} is excluded from {usage}");
            }
        }
    }
    assert_eq!(help["defaults"]["--charset"], "unicode-alphanumeric");
}

#[test]
fn sibling_shapes_each_say_how_they_differ() {
    let help = help_of(&run(&["skill", "install", "--help"]));
    let shapes = help["shapes"].as_array().expect("two shapes");
    assert_eq!(shapes.len(), 2);

    let by_id = |id: &str| {
        shapes
            .iter()
            .find(|shape| shape["id"] == id)
            .unwrap_or_else(|| panic!("missing shape {id}: {help}"))
            .clone()
    };
    let every = by_id("skill-install-every-agent");
    let one = by_id("skill-install-one-agent");
    assert_ne!(every["about"], one["about"]);
    // --skills-dir names a single directory, so it belongs only to the shape
    // that targets a single agent.
    assert!(
        !every["usage"]
            .as_str()
            .unwrap_or_default()
            .contains("--skills-dir"),
        "{every}"
    );
    assert!(
        one["usage"]
            .as_str()
            .unwrap_or_default()
            .contains("[--skills-dir <DIR>]"),
        "{one}"
    );
}

#[test]
fn plain_help_is_not_weaker_than_the_structured_form() {
    let plain = run(&["slugify", "--help", "--output", "plain"]);
    assert!(plain.status.success());
    let text = String::from_utf8(plain.stdout).expect("plain help is UTF-8");

    assert!(text.contains("afslug slugify <TEXT>"), "{text}");
    // Notes and defaults are the two things plain help used to drop.
    assert!(text.contains("Text to slugify"), "{text}");
    assert!(text.contains("--charset=unicode-alphanumeric"), "{text}");
}

#[test]
fn an_unknown_command_is_rejected_without_quoting_it() {
    // `help` was clap's pseudo-command; the registry has no such thing. The
    // rejection classifies it without echoing the token back — `error.code` and
    // the hint are what the caller acts on.
    let pseudo = run(&["help"]);
    assert_eq!(pseudo.status.code(), Some(2));
    assert!(pseudo.stdout.is_empty());
    let event = stderr_json(&pseudo);
    assert_eq!(event["error"]["code"], "cli_unknown_command");
    assert_eq!(event["error"]["message"], "unknown command");
    assert_eq!(
        event["error"]["hint"],
        "run `afslug --help` and choose one registered combination"
    );
    assert!(
        pseudo.stderr.len() < 256,
        "an unknown command must not embed eager help"
    );
}

#[test]
fn output_to_is_honored_once_an_invocation_resolves() {
    let resolved = run(&["validate", "bad/slug", "--output-to", "stdout"]);
    assert_eq!(resolved.status.code(), Some(1));
    assert!(resolved.stderr.is_empty());
    assert_eq!(stdout_json(&resolved)["error"]["code"], "slug_error");
}

#[test]
fn a_rejected_invocation_reports_on_the_diagnostic_stream() {
    // `--output-to stdout` is part of the argv that failed to resolve, so there
    // is no output contract to honor yet; the rejection cannot be routed by the
    // request it is rejecting.
    let output = run(&["--output-to", "stdout"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(stderr_json(&output)["kind"], "error");
}

#[test]
fn docs_render_the_whole_registry_as_markdown() {
    let output = run(&["--docs"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).expect("docs are UTF-8");

    assert!(text.starts_with("# afslug CLI reference"), "{text:.80}");
    for command in ["afslug slugify", "afslug validate", "afslug skill install"] {
        assert!(
            text.contains(command),
            "{command} missing from the reference"
        );
    }
}

#[test]
fn skill_install_bundles_skill_and_agent_asset() {
    let dir = std::env::temp_dir().join(format!("afslug_skill_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let dir_str = dir.to_str().expect("temp path is utf-8");
    let target = [
        "--agent",
        "claude-code",
        "--scope",
        "personal",
        "--skills-dir",
        dir_str,
    ];

    let mut install = vec!["skill", "install"];
    install.extend_from_slice(&target);
    install.push("--force");
    let installed = run(&install);
    assert!(
        installed.status.success(),
        "install failed: {}",
        String::from_utf8_lossy(&installed.stderr)
    );
    let skill_dir = dir.join("agent-first-slug");
    assert!(
        skill_dir.join("SKILL.md").is_file(),
        "SKILL.md must install"
    );
    assert!(
        skill_dir.join("agents").join("openai.yaml").is_file(),
        "the bundled agents/openai.yaml asset must install alongside SKILL.md"
    );

    let mut status = vec!["skill", "status"];
    status.extend_from_slice(&target);
    let value = stdout_json(&run(&status));
    assert_eq!(value["result"]["current_all"], json!(true));

    let mut uninstall = vec!["skill", "uninstall"];
    uninstall.extend_from_slice(&target);
    let removed = run(&uninstall);
    assert!(removed.status.success());
    assert!(
        !skill_dir.exists(),
        "uninstall must remove the skill directory"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
