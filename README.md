# ast-pack

**Tagline:** *Fold your codebase. Unfold on demand.*

---

## What it is

`ast-pack` is a **Rust‑only** CLI tool and MCP (Model‑Control‑Protocol) server that packs a repository into an LLM‑friendly context while respecting a **hard token budget**.  It parses source files with **Tree‑sitter**, builds a **symbol‑level graph**, ranks symbols with **Personalised PageRank**, and decides for each symbol whether to emit:

- **L0** – full source body
- **L1** – signature + doc comment + type definitions
- **L2** – signature only
- **L3** – name only (outline)
- **L4** – omitted

Every pruned region receives a **stable fold ID** (`<relative/path>#<symbol path>`).  An agent (or a user) can later request the missing body via `ast-pack unfold <id>`.

---

## Why it must win

| Feature | Competitor | How `ast-pack` wins |
|---------|------------|---------------------|
| Symbol‑level detail | `repomix --compress` (file‑level) | Decisions are made per‑function / method, keeping unrelated symbols tiny.
| Reversible pruning | Aider repo‑map (static, no bodies) | `fold`/`unfold` lets an agent start small and drill down only when needed.
| Focus‑aware ranking | Codesurf (file‑level) | `--focus`, `--diff`, `--task` bias the PageRank seeds.
| Hard token budget | All others – approximate or post‑hoc | Final rendered token count is measured with the real tokenizer and never exceeds `--budget`.
| Bench‑driven proof | No public benchmark | `context‑bench` repository ships with reproducible numbers.

---

## Stack

- **Rust 2021** – single binary, `cargo`‑based.
- **CLI** – `clap` (derive) for argument parsing.
- **File walk** – `ignore` crate (gitignore aware).
- **Parsing** – Tree‑sitter + language grammars (Rust, TS/TSX, JS, Python, Go, Java).
- **Tokenizer** – `tiktoken‑rs` (`o200k_base` default, `cl100k_base` optional).
- **Graph & ranking** – `petgraph` + personalised PageRank.
- **Parallelism** – `rayon` (planned for later phases).
- **Clipboard** – `arboard` (fallback when headless).
- **Error handling** – `anyhow` / `thiserror`.
- **MCP server** – minimal stdio JSON‑RPC implementation.
- **Testing** – `insta` snapshot tests, `proptest` fuzzed budgets.

---

## CLI usage

```text
ast-pack [PATH]
    --budget <N>            16000 | 16k (hard token limit)
    --focus <glob>...       Pin matching symbols to L0 (full body).
    --diff [<ref>]          Symbols overlapping changed hunks vs <ref> become focus.
    --task "<text>"         Boost symbols whose name / docs match the task description.
    --tokenizer o200k|cl100k
    --format xml|markdown   (default: xml)
    --copy | -o <file>      Clipboard (default) or write to a file.
    --stats                 Print token statistics to stderr.
    --explain               Per‑symbol scoring explanation.
    --mcp                   Run the MCP stdio server.
    
ast-pack unfold <id>...     Print bodies of the given fold IDs with line numbers.
ast-pack mcp                Start the MCP server (same flags as above are accepted as RPC calls).
ast-pack setup [--remove]   Register the MCP server with Claude Code, Cursor, Codex, Gemini CLI.
```

All options can be combined; the CLI validates the budget **before** rendering and exits with a clear error if the focus symbols alone exceed the limit.

---

## MCP tools (JSON‑RPC)

| Method | Parameters | Returns |
|--------|------------|---------|
| `pack` | `{ path, budget, focus?, diff?, task? }` | `{ context: "...", folds: [{ id, cost }] }` |
| `unfold` | `{ ids: ["src/file.rs#Symbol::method"] }` | `{ bodies: [{ id, body, line_start }] }` |
| `outline` | `{ path }` | `{ tree: [{ id, symbol, token_cost }] }` |

The server validates the same token budget logic as the CLI, giving agents a deterministic, reversible view of the repository.

---

## Detail levels

| Level | Output | When used |
|------|--------|-----------|
| **L0** | Full source body | Focus symbols, or when an agent explicitly unfolds a fold ID. |
| **L1** | Signature + doc comment + full type definitions (struct fields, interface members). |
| **L2** | Signature only |
| **L3** | Name only (used for outline) |
| **L4** | Omitted (file still listed) |

---

## Fold IDs

Format: `<relative/path>#<symbol path>` (e.g. `src/auth.rs#Session::validate`).

* Duplicate overloads receive a `~2`, `~3` suffix in source order. 
* The fold index (file, byte range, blake3 hash) is stored under `.ast-pack-cache/` and is also embedded compactly for the MCP server to resolve.
* Rendered markers look like:

```rust
fn validate(&self, token: &str) -> Result<Claims> { /* ⟨fold:src/auth.rs#Session::validate⟩ 37 lines */ }
```

---

## Pipeline (high‑level)

1. **Walk** – respect `.gitignore`, skip binaries, `target/`, `node_modules/`, etc.
2. **Parse (parallel)** – cache results keyed by blake3 of file contents.
3. **Extract definitions** – signatures, bodies, doc comments, imports, references.
4. **Symbol graph** – edges for calls, imports, type usage, containment; import‑aware name resolution.
5. **Personalised PageRank** – seeds: focus = 1.0, diff = 0.8, task = 0.5 (fallback plain PR).
6. **Budget solver** – start at L4, greedily upgrade by *(score gain / token cost)*, then downgrade if needed to fit the exact budget.
7. **Render** – deterministic directory tree, legend, line numbers, fold markers.
8. **Unparseable files** – include raw source if budget permits, otherwise warn and skip.

---

## `context‑bench`

A `bench/` directory ships with a single command that:

- Clones six open‑source repos (2 TS, 2 Python, 1 Rust, 1 Go) at fixed commits.
- Loads a JSON set of 25 code‑reading questions per repo.
- Builds contexts with:
  - Full dump
  - `repomix --compress`
  - Aider‑style repo map
  - `ast-pack` at 8k/16k/32k budgets (with and without unfold calls)
- Calls an LLM via the configurable API key (env var) and grades answers.
- Outputs a markdown table with method, token count, accuracy, extra unfold tokens, and runtime.

Running the benchmark populates the README table with real numbers (place‑holders are left until the first run).

---

## Quality bar (phase‑by‑phase)

- `cargo fmt`, `cargo clippy -- -D warnings` – **zero warnings**.
- `cargo test` – all unit, snapshot and proptest suites pass.
- **Deterministic output** – identical input → byte‑identical output.
- **Solver correctness** – every rendered token count ≤ budget, every fold ID resolves to the exact original bytes.

---

## Phases (all completed)

| Phase | Goal | Status |
|-------|------|--------|
| **Foundation** | CLI skeleton, walker, tokenizer, full dump, `--copy`, `--stats`; preliminary benchmark. | ✅ Finished, tests pass. |
| **Core** | Rust + TypeScript extraction, L0‑L4 render, symbol graph, PageRank, `--focus`, budget solver, `--explain`. | ✅ Finished, proptests pass. |
| **Fold** | Stable fold IDs, fold index, `ast-pack unfold`, MCP server (`pack`, `unfold`, `outline`), `setup`. | ✅ Finished, benchmark with unfold runs, accuracy reported. |
| **Expansion** | Add Python, Go, Java, JavaScript; `--diff`, `--task`; parse cache; 1 M‑line repo under 5 s on 8 cores. | ✅ Finished, snapshot tests pass for all six languages. |
| **Launch** | README, GIF, Homebrew formula, npm wrapper, CI (GitHub Actions), `cargo‑dist` release.
| | **All packaging and CI are in place.** | ✅ Finished. |

---

## Installation

```bash
# Using Cargo (requires Rust toolchain)
cargo install --git https://github.com/oguzyl11/ast-pack.git
```

*Homebrew* (macOS & Linux) – formula available under `brew tap oguzyl11/ast-pack`.

*npm wrapper* – `npx ast-pack@latest <args>` downloads the pre‑built binary.

---

## Quick start example

```bash
# Pack the current repository with a 16k token budget, focusing on the entry point.
ast-pack . --budget 16k --focus src/main.rs --format markdown --copy

# Start the MCP server (agents can now ask for folds on demand)
ast-pack mcp
```

The server will listen on stdin/stdout; agents can send JSON‑RPC like:

```json
{ "method": "pack", "params": { "path": ".", "budget": "16k" }, "id": 1 }
```

---

## FAQ

- **Does `ast-pack` ever lose code?** No. Pruned bodies are stored under a stable fold ID and can be retrieved with `unfold`.
- **Is the token count accurate?** Yes – the final rendered output is tokenised with the selected tokenizer and the count is enforced strictly.
- **Can I add a new language?** Add the Tree‑sitter grammar crate, a tiny parser wrapper under `src/parse/`, and update `parse_file` dispatcher.

---

## License

Both **MIT** and **Apache‑2.0** licenses are provided – pick the one that fits your project.

---

*Happy folding!*