#!/usr/bin/env node
"use strict";

const { spawn } = require("child_process");
const path = require("path");
const fs = require("fs");

const candidates = [
  process.env.AST_PACK_BIN,
  path.join(__dirname, "..", "..", "target", "release", process.platform === "win32" ? "ast-pack.exe" : "ast-pack"),
  path.join(__dirname, "..", "..", "target", "debug", process.platform === "win32" ? "ast-pack.exe" : "ast-pack"),
].filter(Boolean);

function existing() {
  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) {
      return candidate;
    }
  }
  return process.platform === "win32" ? "ast-pack.exe" : "ast-pack";
}

const child = spawn(existing(), process.argv.slice(2), { stdio: "inherit" });
child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code == null ? 1 : code);
});
child.on("error", (err) => {
  console.error(
    "ast-pack binary was not found. Install it with `cargo install --git https://github.com/oguzyl11/ast-pack.git` or set AST_PACK_BIN.\n" +
      err.message
  );
  process.exit(1);
});
