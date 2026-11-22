//! Local context benchmark.
//!
//! The corpus is six small repositories (2 TypeScript, 2 Python, 1 Rust, 1 Go),
//! 25 questions each. Answers sit in function bodies, not in signatures.
//! Without an API key this measures retrieval: whether the packed context
//! contains the answer. Set `AST_PACK_API_KEY` only when you want the optional
//! LLM judge described in `bench/judge_prompt.txt`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use ast_pack::{
    fold::fold_ids_in, pack, unfold, utils::task_terms, PackMode, PackOptions, Tokenizer,
};

const NAMES: &[&str] = &[
    "charge_invoice",
    "retry_payment",
    "lock_account",
    "purge_session",
    "quote_tax",
    "route_webhook",
    "seal_audit",
    "open_drawer",
    "merge_cart",
    "split_refund",
    "page_cursor",
    "hash_token",
    "lease_worker",
    "drain_queue",
    "mark_shipped",
    "apply_coupon",
    "rotate_key",
    "bind_device",
    "score_risk",
    "close_batch",
    "lift_hold",
    "sync_ledger",
    "stamp_receipt",
    "warm_cache",
    "drop_orphan",
];

const REPOS: &[(&str, &str)] = &[
    ("ts-shop", "ts"),
    ("ts-notes", "ts"),
    ("py-billing", "py"),
    ("py-inventory", "py"),
    ("rs-auth", "rs"),
    ("go-queue", "go"),
];

struct Question {
    name: String,
    question: String,
    answer: String,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let materialize = args.iter().any(|arg| arg == "--materialize");
    let run_bench = !materialize || args.iter().any(|arg| arg == "--run");
    let root = PathBuf::from("bench");
    if materialize || !root.join("corpus").join("ts-shop").exists() {
        materialize_corpus(&root);
        println!("wrote {}", root.display());
    }
    if !run_bench {
        return;
    }
    match run(&root) {
        Ok(markdown) => {
            println!("{markdown}");
            let _ = fs::write(root.join("RESULTS.md"), &markdown);
        }
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    }
}

fn materialize_corpus(root: &Path) {
    for (name, ext) in REPOS {
        let dir = root.join("corpus").join(name).join("src");
        fs::create_dir_all(&dir).unwrap();
        let questions = questions_for(name);
        fs::write(
            dir.join(format!("lib.{ext}")),
            render_source(ext, &questions),
        )
        .unwrap();
        fs::create_dir_all(root.join("questions")).unwrap();
        let json = serde_json::json!({
            "repo": name,
            "questions": questions.iter().map(|q| serde_json::json!({
                "id": q.name,
                "question": q.question,
                "answer": q.answer,
            })).collect::<Vec<_>>()
        });
        fs::write(
            root.join("questions").join(format!("{name}.json")),
            serde_json::to_string_pretty(&json).unwrap(),
        )
        .unwrap();
    }
}

fn questions_for(repo: &str) -> Vec<Question> {
    NAMES
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let answer = format!("zkq-{repo}-{index:02}");
            Question {
                name: (*name).to_string(),
                question: format!("What value does {name} record?"),
                answer,
            }
        })
        .collect()
}

fn render_source(ext: &str, questions: &[Question]) -> String {
    let mut out = String::new();
    if ext == "go" {
        out.push_str("package queue\n\n");
    }
    for question in questions {
        out.push_str(&render_fn(ext, &question.name, &question.answer));
        out.push('\n');
    }
    out
}

fn render_fn(ext: &str, name: &str, answer: &str) -> String {
    let pad = (0..40)
        .map(|i| match ext {
            "py" => format!("    lane = {i}\n    lane = lane + 1"),
            "go" => format!("    lane = lane + {i}"),
            "rs" => format!("    let mut lane = {i};\n    lane = lane.wrapping_add(1);"),
            _ => format!("  let lane = {i};\n  lane = lane + 1;"),
        })
        .collect::<Vec<_>>()
        .join("\n");
    match ext {
        "py" => format!(
            "def {name}():\n    \"\"\"Record the configured value.\"\"\"\n    lane = 0\n{pad}\n    return \"{answer}\"\n"
        ),
        "go" => format!(
            "// Record the configured value.\nfunc {name}() string {{\n    lane := 0\n{pad}\n    return \"{answer}\"\n}}\n"
        ),
        "rs" => format!(
            "/// Record the configured value.\npub fn {name}() -> &'static str {{\n    let mut lane = 0;\n{pad}\n    let _ = lane;\n    \"{answer}\"\n}}\n"
        ),
        _ => format!(
            "/** Record the configured value. */\nexport function {name}(): string {{\n  let lane = 0;\n{pad}\n  return \"{answer}\";\n}}\n"
        ),
    }
}

#[derive(Clone)]
struct Score {
    method: String,
    tokens: usize,
    extra: usize,
    hits: usize,
    total: usize,
    millis: u128,
}

fn run(root: &Path) -> Result<String, String> {
    let tokenizer = Tokenizer::new("o200k").map_err(|err| err.to_string())?;
    let mut scores = Vec::new();
    for (name, _) in REPOS {
        let repo = root.join("corpus").join(name);
        let questions = questions_for(name);
        scores.extend(score_repo(&repo, &questions, &tokenizer)?);
    }
    Ok(render_table(&collapse(&scores)))
}

fn score_repo(
    repo: &Path,
    questions: &[Question],
    tokenizer: &Tokenizer,
) -> Result<Vec<Score>, String> {
    let mut rows = Vec::new();
    rows.push(pack_row(
        repo,
        questions,
        tokenizer,
        "full dump",
        PackMode::Full,
        None,
        false,
    )?);
    rows.push(compress_row(repo, questions, tokenizer)?);
    rows.push(pack_row(
        repo,
        questions,
        tokenizer,
        "name map",
        PackMode::Names,
        None,
        false,
    )?);
    for budget in [8_000usize, 16_000, 32_000] {
        let label = format!("ast-pack {budget}");
        rows.push(pack_row(
            repo,
            questions,
            tokenizer,
            &label,
            PackMode::Auto,
            Some(budget),
            false,
        )?);
        let unfold_label = format!("ast-pack {budget} + unfold");
        rows.push(pack_row(
            repo,
            questions,
            tokenizer,
            &unfold_label,
            PackMode::Auto,
            Some(budget),
            true,
        )?);
    }
    Ok(rows)
}

fn pack_row(
    repo: &Path,
    questions: &[Question],
    tokenizer: &Tokenizer,
    label: &str,
    mode: PackMode,
    budget: Option<usize>,
    with_unfold: bool,
) -> Result<Score, String> {
    let started = Instant::now();
    let mut options = PackOptions::new(repo);
    options.mode = mode;
    options.budget = budget;
    options.cache = false;
    options.write_folds = false;
    let packed = pack(&options).map_err(|err| err.to_string())?;
    let mut hits = 0;
    let mut extra_sum = 0usize;
    for question in questions {
        let (context, extra) = if with_unfold {
            unfold_for(repo, &packed.text, &question.question, tokenizer)
        } else {
            (packed.text.clone(), 0)
        };
        if context.contains(&question.answer) {
            hits += 1;
        }
        extra_sum += extra;
    }
    Ok(Score {
        method: label.to_string(),
        tokens: packed.tokens,
        extra: if with_unfold {
            extra_sum / questions.len().max(1)
        } else {
            0
        },
        hits,
        total: questions.len(),
        millis: started.elapsed().as_millis(),
    })
}

fn unfold_for(repo: &Path, packed: &str, question: &str, tokenizer: &Tokenizer) -> (String, usize) {
    let terms = task_terms(question);
    let mut ranked: Vec<(i32, String)> = fold_ids_in(packed)
        .into_iter()
        .map(|id| {
            let lower = id.to_lowercase();
            let score = terms
                .iter()
                .filter(|term| lower.contains(term.as_str()))
                .count() as i32;
            (score, id)
        })
        .filter(|(score, _)| *score > 0)
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    ranked.truncate(3);
    let mut extra = String::new();
    for (_, id) in ranked {
        if let Ok(body) = unfold(repo, &[id]) {
            extra.push_str(&body.text);
            extra.push('\n');
        }
    }
    let tokens = tokenizer.count(&extra);
    (format!("{packed}\n{extra}"), tokens)
}

fn compress_row(
    repo: &Path,
    questions: &[Question],
    tokenizer: &Tokenizer,
) -> Result<Score, String> {
    let started = Instant::now();
    let mut text = String::new();
    for entry in walkdir(repo) {
        let source = fs::read_to_string(&entry).map_err(|err| err.to_string())?;
        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("//")
                || trimmed.starts_with('#')
                || trimmed.starts_with("/*")
                || trimmed.starts_with('*')
            {
                continue;
            }
            text.push_str(trimmed);
            text.push('\n');
        }
    }
    let tokens = tokenizer.count(&text);
    let hits = questions
        .iter()
        .filter(|q| text.contains(&q.answer))
        .count();
    Ok(Score {
        method: "repomix-style compress".into(),
        tokens,
        extra: 0,
        hits,
        total: questions.len(),
        millis: started.elapsed().as_millis(),
    })
}

fn walkdir(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

fn collapse(rows: &[Score]) -> Vec<Score> {
    let mut grouped: Vec<Score> = Vec::new();
    for row in rows {
        if let Some(existing) = grouped.iter_mut().find(|item| item.method == row.method) {
            existing.tokens += row.tokens;
            existing.extra += row.extra;
            existing.hits += row.hits;
            existing.total += row.total;
            existing.millis += row.millis;
        } else {
            grouped.push(row.clone());
        }
    }
    let n = REPOS.len();
    for row in &mut grouped {
        row.tokens /= n;
        row.extra /= n;
        row.millis /= n as u128;
    }
    grouped
}

fn render_table(rows: &[Score]) -> String {
    let mut out = String::from(
        "# context-bench\n\n\
Local corpus: 6 repositories, 25 questions each. A hit means the answer string is present in the context. \
This is retrieval coverage, not an LLM accuracy score. The optional judge prompt is `bench/judge_prompt.txt`.\n\n\
| Method | Avg tokens | Avg unfold tokens | Coverage | Avg ms |\n\
| --- | ---: | ---: | ---: | ---: |\n",
    );
    for row in rows {
        let coverage = if row.total == 0 {
            0.0
        } else {
            (row.hits as f64) * 100.0 / (row.total as f64)
        };
        out.push_str(&format!(
            "| {} | {} | {} | {:.1}% | {} |\n",
            row.method, row.tokens, row.extra, coverage, row.millis
        ));
    }
    out.push_str(
        "\nFull dump and the repomix-style strip keep every body, so coverage is high and so is the token count. \
The name map keeps almost none of the answers. ast-pack at a small budget shows signatures and fold ids; \
unfold spends extra tokens only on the bodies the question names.\n",
    );
    out
}
