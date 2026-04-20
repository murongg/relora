#!/usr/bin/env node

const { existsSync } = require("node:fs");
const path = require("node:path");
const { spawn } = require("node:child_process");

const packageRoot = path.resolve(__dirname, "..");
const executableName = process.platform === "win32" ? "relora.exe" : "relora";
const executablePath = path.join(packageRoot, "dist", "bin", executableName);

if (!existsSync(executablePath)) {
  console.error(
    [
      "Relora is not installed correctly yet.",
      "The npm package could not find the downloaded runtime bundle.",
      "Try reinstalling with `npm install -g relora`.",
    ].join("\n"),
  );
  process.exit(1);
}

const child = spawn(executablePath, process.argv.slice(2), {
  stdio: "inherit",
  windowsHide: false,
});

child.on("error", (error) => {
  console.error(`Failed to launch Relora: ${error.message}`);
  process.exit(1);
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 0);
});
