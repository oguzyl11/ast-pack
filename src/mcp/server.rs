use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use serde_json::{json, Value};

use crate::error::PackError;
use crate::fold;
use crate::mode::PackMode;
use crate::pipeline::{self, PackOptions};
use crate::render::Format;
use crate::utils::parse_budget;

pub fn run() -> Result<(), PackError> {
    let stdin = io::stdin();
    let mut reader = stdin.lock();
    let mut stdout = io::stdout();
    while let Some(incoming) = read_message(&mut reader)? {
        if let Some(response) = handle_request(&incoming.body) {
            write_message(&mut stdout, &response, incoming.framed)?;
        }
    }
    Ok(())
}

struct Incoming {
    body: String,
    framed: bool,
}

fn read_message(reader: &mut impl BufRead) -> Result<Option<Incoming>, PackError> {
    let mut line = String::new();
    let read = reader.read_line(&mut line)?;
    if read == 0 {
        return Ok(None);
    }
    if line.to_ascii_lowercase().starts_with("content-length:") {
        let len: usize = line
            .split(':')
            .nth(1)
            .unwrap_or("0")
            .trim()
            .parse()
            .map_err(|_| PackError::msg("invalid Content-Length"))?;
        loop {
            let mut header = String::new();
            let n = reader.read_line(&mut header)?;
            if n == 0 || header == "\n" || header == "\r\n" {
                break;
            }
        }
        let mut buf = vec![0u8; len];
        reader.read_exact(&mut buf)?;
        let body = String::from_utf8_lossy(&buf).to_string();
        return Ok(Some(Incoming { body, framed: true }));
    }
    if line.trim().is_empty() {
        return read_message(reader);
    }
    Ok(Some(Incoming {
        body: line,
        framed: false,
    }))
}

fn write_message(out: &mut impl Write, body: &str, framed: bool) -> Result<(), PackError> {
    if framed {
        write!(out, "Content-Length: {}\r\n\r\n", body.len())?;
        out.write_all(body.as_bytes())?;
    } else {
        writeln!(out, "{body}")?;
    }
    out.flush()?;
    Ok(())
}

/// Handle one JSON-RPC body. Notifications return `None`.
pub fn handle_request(body: &str) -> Option<String> {
    let value: Value = match serde_json::from_str(body) {
        Ok(value) => value,
        Err(err) => {
            return Some(error_response(
                Value::Null,
                -32700,
                &format!("parse error: {err}"),
            ));
        }
    };
    let id = value.get("id").cloned().unwrap_or(Value::Null);
    let method = value.get("method").and_then(Value::as_str).unwrap_or("");
    if method.starts_with("notifications/")
        || value.get("id").is_none()
        || id.is_null() && method.starts_with("notifications/")
    {
        return None;
    }
    if method.is_empty() {
        return Some(error_response(id, -32600, "missing method"));
    }
    let params = value.get("params").cloned().unwrap_or(Value::Null);
    let result = dispatch(method, &params);
    match result {
        Dispatch::Notification => None,
        Dispatch::Reply(result) => Some(ok_response(id, result)),
        Dispatch::Fail(message) => Some(error_response(id, -32000, &message)),
    }
}

enum Dispatch {
    Notification,
    Reply(Value),
    Fail(String),
}

fn dispatch(method: &str, params: &Value) -> Dispatch {
    match method {
        "initialize" => Dispatch::Reply(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": "ast-pack", "version": env!("CARGO_PKG_VERSION") }
        })),
        "notifications/initialized" | "initialized" => Dispatch::Notification,
        "ping" => Dispatch::Reply(json!({})),
        "tools/list" => Dispatch::Reply(json!({ "tools": tools() })),
        "tools/call" => match tool_call(params) {
            Ok(value) => Dispatch::Reply(value),
            Err(err) => Dispatch::Reply(json!({
                "content": [{ "type": "text", "text": err.to_string() }],
                "isError": true
            })),
        },
        "pack" => match pack_value(params) {
            Ok(value) => Dispatch::Reply(value),
            Err(err) => Dispatch::Fail(err.to_string()),
        },
        "unfold" => match unfold_value(params) {
            Ok(value) => Dispatch::Reply(value),
            Err(err) => Dispatch::Fail(err.to_string()),
        },
        "outline" => match outline_value(params) {
            Ok(value) => Dispatch::Reply(value),
            Err(err) => Dispatch::Fail(err.to_string()),
        },
        other => Dispatch::Fail(format!("unknown method `{other}`")),
    }
}

fn tools() -> Value {
    json!([
        {
            "name": "pack",
            "description": "Fold a repository into a hard token budget. Pruned bodies stay addressable by fold id. Pass budget as 16000 or \"16k\".",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "budget": { "description": "Token budget, for example 16000 or 16k" },
                    "focus": { "type": "array", "items": { "type": "string" } },
                    "diff": { "type": "string" },
                    "task": { "type": "string" },
                    "format": { "type": "string", "enum": ["xml", "markdown"] }
                },
                "required": ["path"]
            }
        },
        {
            "name": "unfold",
            "description": "Return the original body of one or more fold ids, with line numbers. If the file changed, the current body is returned and the text says so.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "ids": { "type": "array", "items": { "type": "string" } },
                    "id": { "type": "string" }
                }
            }
        },
        {
            "name": "outline",
            "description": "List symbols and the token cost of their full bodies.",
            "inputSchema": {
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }
        }
    ])
}

fn tool_call(params: &Value) -> Result<Value, PackError> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or_else(|| PackError::msg("tools/call is missing a name"))?;
    let args = params.get("arguments").cloned().unwrap_or(json!({}));
    let value = match name {
        "pack" => pack_value(&args)?,
        "unfold" => unfold_value(&args)?,
        "outline" => outline_value(&args)?,
        other => return Err(PackError::msg(format!("unknown tool `{other}`"))),
    };
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    Ok(json!({
        "content": [{ "type": "text", "text": text }],
        "isError": false
    }))
}

fn pack_value(params: &Value) -> Result<Value, PackError> {
    let path = params.get("path").and_then(Value::as_str).unwrap_or(".");
    let mut options = PackOptions::new(PathBuf::from(path));
    options.budget = params.get("budget").and_then(budget_value);
    options.focus = string_list(params.get("focus"));
    options.diff = params
        .get("diff")
        .and_then(Value::as_str)
        .map(str::to_string);
    options.task = params
        .get("task")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let Some(format) = params.get("format").and_then(Value::as_str) {
        options.format = Format::parse(format)?;
    }
    options.mode = PackMode::Auto;
    options.compute_full = false;
    let packed = pipeline::pack(&options)?;
    Ok(json!({
        "context": packed.text,
        "tokens": packed.tokens,
        "budget": options.budget,
        "warnings": packed.warnings,
        "folds": packed.folds.iter().map(|fold| json!({
            "id": fold.id,
            "level": fold.level,
            "cost": fold.cost,
            "line_start": fold.line_start,
            "line_end": fold.line_end,
        })).collect::<Vec<_>>()
    }))
}

fn unfold_value(params: &Value) -> Result<Value, PackError> {
    let path = params.get("path").and_then(Value::as_str).unwrap_or(".");
    let mut ids = string_list(params.get("ids"));
    if let Some(id) = params.get("id").and_then(Value::as_str) {
        ids.push(id.to_string());
    }
    let unfolded = fold::unfold(PathBuf::from(path).as_path(), &ids)?;
    Ok(json!({
        "bodies": [{
            "text": unfolded.text,
            "warnings": unfolded.warnings,
        }]
    }))
}

fn outline_value(params: &Value) -> Result<Value, PackError> {
    let path = params.get("path").and_then(Value::as_str).unwrap_or(".");
    pipeline::outline(PathBuf::from(path).as_path())
}

fn budget_value(value: &Value) -> Option<usize> {
    match value {
        Value::Number(number) => number.as_u64().map(|n| n as usize).filter(|n| *n > 0),
        Value::String(text) => parse_budget(text).ok(),
        _ => None,
    }
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect(),
        Some(Value::String(text)) => vec![text.clone()],
        _ => Vec::new(),
    }
}

fn ok_response(id: Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

fn error_response(id: Value, code: i32, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
    .to_string()
}
