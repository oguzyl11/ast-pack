use std::fs;
use std::path::Path;
use std::process::Command;

use ast_pack::{pack, unfold, Format, PackMode, PackOptions, Tokenizer};

fn write(dir: &Path, rel: &str, body: &str) {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, body).unwrap();
}

#[test]
fn parses_six_languages() {
    let cases = [
        (
            "session.rs",
            r#"
/// Session token helper.
pub struct Session {
    pub user: String,
}

impl Session {
    /// Check the bearer token.
    pub fn validate(&self, token: &str) -> bool {
        token == "secret-token"
    }
}

pub fn retry_limit() -> i32 {
    5
}
"#,
            "retry_limit",
        ),
        (
            "billing.py",
            r#"
class Session:
    """A signed session."""
    def validate(self, token):
        """Check the bearer token."""
        return token == "secret-token"

def retry_limit():
    """How many times to retry."""
    return 5
"#,
            "retry_limit",
        ),
        (
            "auth.js",
            r#"
/** How many times to retry. */
function retry_limit() {
  return 5;
}

class Session {
  validate(token) {
    return token === "secret-token";
  }
}
"#,
            "retry_limit",
        ),
        (
            "auth.ts",
            r#"
export interface Session {
  user: string;
}

export function retry_limit(): number {
  return 5;
}

export class Auth {
  validate(token: string): boolean {
    return token === "secret-token";
  }
}
"#,
            "retry_limit",
        ),
        (
            "auth.go",
            r#"
package auth

type Session struct {
    User string
}

func (s *Session) Validate(token string) bool {
    return token == "secret-token"
}

func RetryLimit() int {
    return 5
}
"#,
            "RetryLimit",
        ),
        (
            "Auth.java",
            r#"
public class Auth {
    /** How many times to retry. */
    public int retryLimit() {
        return 5;
    }
}
"#,
            "retryLimit",
        ),
    ];

    for (name, source, expected) in cases {
        let (symbols, _) = ast_pack::parse::parse_source(name, source).unwrap_or_else(|err| {
            panic!(
                "{name}: {err}\n{}",
                ast_pack::parse::debug_sexp(name, source)
            )
        });
        let names: Vec<_> = symbols.iter().map(|s| s.name.clone()).collect();
        assert!(
            names.iter().any(|n| n == expected),
            "{name} missing {expected}, got {names:?}\n{}",
            ast_pack::parse::debug_sexp(name, source)
        );
    }
}

#[test]
fn overload_suffix_is_stable() {
    let source = "fn dup() { let _ = 1; }\nfn dup() { let _ = 2; }\n";
    let (symbols, _) = ast_pack::parse::parse_source("t.rs", source).unwrap();
    let mut ids: Vec<_> = symbols.iter().map(|s| s.id.fold_id()).collect();
    ids.sort();
    assert_eq!(ids, vec!["t.rs#dup".to_string(), "t.rs#dup~2".to_string()]);
}

#[test]
fn budget_is_hard_and_output_is_deterministic() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "src/lib.rs",
        &format!(
            "pub fn alpha() {{\n{}\n}}\npub fn beta() {{\n{}\n}}\n",
            "let _ = 1;\n".repeat(40),
            "let _ = 2;\n".repeat(40)
        ),
    );
    let mut options = PackOptions::new(dir.path());
    options.budget = Some(400);
    options.cache = false;
    options.write_folds = true;
    let first = pack(&options).unwrap();
    let second = pack(&options).unwrap();
    assert!(first.tokens <= 400, "tokens {}", first.tokens);
    assert_eq!(first.text, second.text);
    assert!(first.text.contains("⟨fold:"));
}

#[test]
fn focus_over_budget_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "src/lib.rs",
        &format!("pub fn huge() {{\n{}\n}}\n", "let _ = 1;\n".repeat(200)),
    );
    let mut options = PackOptions::new(dir.path());
    options.budget = Some(80);
    options.focus = vec!["src/lib.rs".into()];
    options.cache = false;
    options.write_folds = false;
    let err = pack(&options).unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains("focus") || message.contains("budget"),
        "{message}"
    );
}

#[test]
fn task_prefers_the_matching_symbol() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "src/lib.rs",
        &format!(
            "pub fn retry_payment() {{\n{}\n}}\npub fn unrelated_widget() {{\n{}\n}}\n",
            "let _ = 1;\n".repeat(30),
            "let _ = 2;\n".repeat(30)
        ),
    );
    let mut options = PackOptions::new(dir.path());
    options.budget = Some(250);
    options.task = Some("retry payment".into());
    options.cache = false;
    options.write_folds = false;
    let packed = pack(&options).unwrap();
    assert!(packed.tokens <= 250, "{}", packed.tokens);
    let retry = packed
        .decisions
        .iter()
        .find(|d| d.id.contains("retry_payment"))
        .unwrap();
    let other = packed
        .decisions
        .iter()
        .find(|d| d.id.contains("unrelated_widget"))
        .unwrap();
    assert!(
        retry.level.value() >= other.level.value(),
        "retry {:?} other {:?}",
        retry.level,
        other.level
    );
}

#[test]
fn unfold_returns_the_original_body() {
    let dir = tempfile::tempdir().unwrap();
    let source = "pub fn retry_limit() -> i32 {\n    5\n}\n";
    write(dir.path(), "src/lib.rs", source);
    let mut options = PackOptions::new(dir.path());
    options.budget = Some(200);
    options.cache = false;
    let packed = pack(&options).unwrap();
    let id = packed
        .folds
        .iter()
        .find(|f| f.id.contains("retry_limit"))
        .unwrap()
        .id
        .clone();
    let unfolded = unfold(dir.path(), &[id]).unwrap();
    assert!(unfolded.warnings.is_empty());
    assert!(unfolded.text.contains("5"));
    assert!(unfolded.text.contains("retry_limit"));
}

#[test]
fn changed_file_warns_on_unfold() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "src/lib.rs",
        "pub fn retry_limit() -> i32 {\n    5\n}\n",
    );
    let mut options = PackOptions::new(dir.path());
    options.cache = false;
    options.budget = Some(200);
    let packed = pack(&options).unwrap();
    let id = packed
        .folds
        .iter()
        .find(|f| f.id.contains("retry_limit"))
        .unwrap()
        .id
        .clone();
    write(
        dir.path(),
        "src/lib.rs",
        "pub fn retry_limit() -> i32 {\n    9\n}\n",
    );
    let unfolded = unfold(dir.path(), std::slice::from_ref(&id)).unwrap();
    assert!(!unfolded.warnings.is_empty(), "{:?}", unfolded.warnings);
    assert!(unfolded.text.contains('9'));
}

#[test]
fn diff_promotes_the_edited_symbol() {
    let dir = tempfile::tempdir().unwrap();
    let lib = "pub fn kept() -> i32 { 1 }\npub fn edited() -> i32 { 1 }\n";
    write(dir.path(), "src/lib.rs", lib);
    git(dir.path(), &["init"]);
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-m", "init"]);
    write(
        dir.path(),
        "src/lib.rs",
        "pub fn kept() -> i32 { 1 }\npub fn edited() -> i32 { 2 }\n",
    );
    let mut options = PackOptions::new(dir.path());
    options.diff = Some("HEAD".into());
    options.budget = None;
    options.cache = false;
    options.write_folds = false;
    let packed = pack(&options).unwrap();
    let edited = packed
        .decisions
        .iter()
        .find(|d| d.id.ends_with("#edited"))
        .unwrap();
    assert_eq!(edited.level, ast_pack::model::DetailLevel::L0);
}

#[test]
fn mcp_lists_tools_and_packs() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "src/lib.rs",
        "pub fn retry_limit() -> i32 { 5 }\n",
    );
    let init = ast_pack::mcp::server::handle_request(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
    )
    .unwrap();
    assert!(init.contains("ast-pack"));
    let tools = ast_pack::mcp::server::handle_request(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
    )
    .unwrap();
    assert!(tools.contains("unfold"));
    assert!(ast_pack::mcp::server::handle_request(
        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#
    )
    .is_none());
    let request = format!(
        r#"{{"jsonrpc":"2.0","id":3,"method":"pack","params":{{"path":"{}","budget":"2k"}}}}"#,
        dir.path().display().to_string().replace('\\', "\\\\")
    );
    let response = ast_pack::mcp::server::handle_request(&request).unwrap();
    assert!(response.contains("retry_limit") || response.contains("context"));
}

#[test]
fn setup_round_trip_restores_files() {
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    fs::write(home.path().join(".claude.json"), "{\"numStartups\":1}\n").unwrap();
    let notes = ast_pack::setup::apply(home.path(), project.path(), false).unwrap();
    assert!(notes.iter().any(|n| n.contains("registered")));
    let cursor = fs::read_to_string(project.path().join(".cursor").join("mcp.json")).unwrap();
    assert!(cursor.contains("ast-pack"));
    let claude = fs::read_to_string(home.path().join(".claude.json")).unwrap();
    assert!(claude.contains("numStartups"));
    assert!(claude.contains("ast-pack"));
    ast_pack::setup::apply(home.path(), project.path(), true).unwrap();
    let restored = fs::read_to_string(home.path().join(".claude.json")).unwrap();
    assert!(restored.contains("numStartups"));
    assert!(!restored.contains("ast-pack"));
    assert!(!project.path().join(".cursor").join("mcp.json").exists());
}

#[test]
fn stats_report_matches_packed_token_savings() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "src/lib.rs",
        "pub fn retry_limit() -> i32 { 5 }\n",
    );
    let mut options = PackOptions::new(dir.path());
    options.budget = Some(400);
    options.compute_full = true;
    options.cache = false;
    options.write_folds = false;
    let packed = pack(&options).unwrap();
    let full = packed.tokens_full.expect("full token count");
    let saved = full.saturating_sub(packed.tokens);
    let report = ast_pack::cli::format_stats(
        packed.tokens,
        packed.tokens_full,
        options.budget,
        packed.level_counts,
    );
    let expected = ast_pack::cli::savings_line(saved);
    assert!(report.contains(&format!("tokens: {} / full {full}", packed.tokens)));
    assert!(report.contains(&expected));
    assert!(expected.contains("~$"));
    assert!(expected.contains("(Claude Sonnet)"));
    assert!(expected.contains("(GPT-4o)"));
}

#[test]
fn setup_writes_agent_files_and_remove_cleans_them() {
    let home = tempfile::tempdir().unwrap();
    let project = tempfile::tempdir().unwrap();
    let skill = project
        .path()
        .join(".claude")
        .join("skills")
        .join("ast-pack")
        .join("SKILL.md");
    let rule = project
        .path()
        .join(".cursor")
        .join("rules")
        .join("ast-pack.mdc");
    let prior = "keep this local rule\n";
    fs::create_dir_all(rule.parent().unwrap()).unwrap();
    fs::write(&rule, prior).unwrap();

    ast_pack::setup::apply(home.path(), project.path(), false).unwrap();
    let skill_text = fs::read_to_string(&skill).unwrap();
    let rule_text = fs::read_to_string(&rule).unwrap();
    for text in [&skill_text, &rule_text] {
        let lines = text.lines().count();
        assert!(
            (15..=20).contains(&lines),
            "expected 15-20 lines, got {lines}"
        );
        assert!(text.contains(
            "Always call `pack` with `--budget` and `--task` before reading whole files, and call `unfold` on `⟨fold:...⟩` IDs only when you need the exact implementation body."
        ));
    }
    assert!(skill_text.contains("name: ast-pack"));
    assert!(rule_text.contains("alwaysApply: true"));

    ast_pack::setup::apply(home.path(), project.path(), true).unwrap();
    assert!(!skill.exists());
    assert_eq!(fs::read_to_string(&rule).unwrap(), prior);
    assert!(!rule.with_extension("mdc.ast-pack-bak").exists());
}

#[test]
fn markdown_mentions_the_fold_id() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "src/lib.rs",
        "pub fn retry_limit() -> i32 { 5 }\n",
    );
    let mut options = PackOptions::new(dir.path());
    options.format = Format::Markdown;
    options.budget = None;
    options.cache = false;
    options.write_folds = false;
    let packed = pack(&options).unwrap();
    assert!(packed.text.contains("retry_limit"));
    assert!(packed.tokens > 0);
}

proptest::proptest! {
    #![proptest_config(proptest::test_runner::Config::with_cases(8))]
    #[test]
    fn random_budgets_never_overflow(budget in 40usize..900, funcs in 1usize..6) {
        let dir = tempfile::tempdir().unwrap();
        let mut source = String::new();
        for i in 0..funcs {
            source.push_str(&format!("pub fn fun_{i}() {{\n"));
            source.push_str(&"let _ = 1;\n".repeat(12));
            source.push_str("}\n");
        }
        write(dir.path(), "src/lib.rs", &source);
        let mut options = PackOptions::new(dir.path());
        options.budget = Some(budget);
        options.cache = false;
        options.write_folds = false;
        options.mode = PackMode::Auto;
        match pack(&options) {
            Ok(packed) => assert!(packed.tokens <= budget, "{} > {budget}", packed.tokens),
            Err(err) => {
                let message = err.to_string();
                assert!(message.contains("budget") || message.contains("focus"), "{message}");
            }
        }
    }
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "ast-pack")
        .env("GIT_AUTHOR_EMAIL", "ast-pack@example.com")
        .env("GIT_COMMITTER_NAME", "ast-pack")
        .env("GIT_COMMITTER_EMAIL", "ast-pack@example.com")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?}");
}

#[test]
fn tokenizer_rejects_unknown_names() {
    assert!(Tokenizer::new("nope").is_err());
}
