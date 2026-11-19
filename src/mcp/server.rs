// src/mcp/server.rs
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};

/// Minimal MCP JSON‑RPC‑like request.
#[derive(Debug, Serialize, Deserialize)]
struct Request {
    method: String,
    params: serde_json::Value,
    id: u64,
}

/// Minimal MCP JSON‑RPC‑like response.
#[derive(Debug, Serialize, Deserialize)]
struct Response {
    result: serde_json::Value,
    id: u64,
    error: Option<String>,
}

/// Run a stdio‑based MCP server. It reads line‑delimited JSON requests from stdin
/// and writes JSON responses to stdout.
pub fn run_server() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        // Parse request
        let req: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let err = Response {
                    result: serde_json::json!(null),
                    id: 0,
                    error: Some(format!("invalid request: {}", e)),
                };
                writeln!(stdout, "{}", serde_json::to_string(&err)?)?;
                stdout.flush()?;
                continue;
            }
        };

        // Dispatch method
        let resp = match req.method.as_str() {
            "pack" => handle_pack(req.params, req.id),
            "unfold" => handle_unfold(req.params, req.id),
            "outline" => handle_outline(req.params, req.id),
            _ => Response {
                result: serde_json::json!(null),
                id: req.id,
                error: Some(format!("unknown method `{}`", req.method)),
            },
        };

        writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
        stdout.flush()?;
    }

    Ok(())
}

// -----------------------------------------------------------
// Placeholder handlers – will be fleshed out in later phases.
// -----------------------------------------------------------
fn handle_pack(_params: serde_json::Value, id: u64) -> Response {
    Response {
        result: serde_json::json!({
            "status": "ok",
            "message": "pack not implemented yet"
        }),
        id,
        error: None,
    }
}

fn handle_unfold(_params: serde_json::Value, id: u64) -> Response {
    Response {
        result: serde_json::json!({
            "status": "ok",
            "message": "unfold not implemented yet"
        }),
        id,
        error: None,
    }
}

fn handle_outline(_params: serde_json::Value, id: u64) -> Response {
    Response {
        result: serde_json::json!({
            "status": "ok",
            "message": "outline not implemented yet"
        }),
        id,
        error: None,
    }
}

