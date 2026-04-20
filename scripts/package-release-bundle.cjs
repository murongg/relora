#!/usr/bin/env node

const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const workspaceRoot = path.resolve(__dirname, "..");

main();

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    printHelp();
    process.exit(0);
  }

  if (!args.platform || !args.arch) {
    printHelp("Both --platform and --arch are required.");
    process.exit(1);
  }

  const target = resolveTarget(args.platform, args.arch);
  const version = args.version ?? readWorkspaceVersion();
  const inputDir = path.resolve(workspaceRoot, args.inputDir ?? "target/release");
  const outputDir = path.resolve(workspaceRoot, args.outputDir ?? "dist/release-bundles");
  const assetName = `relora-v${version}-${target.platform}-${target.arch}.tar.gz`;
  const outputPath = path.join(outputDir, assetName);

  ensureTarAvailable();
  fs.mkdirSync(outputDir, { recursive: true });

  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "relora-release-bundle-"));
  try {
    const stagingDir = path.join(tempRoot, `relora-v${version}-${target.platform}-${target.arch}`);
    fs.mkdirSync(stagingDir, { recursive: true });

    for (const fileName of bundleBinaryNames(target)) {
      const source = path.join(inputDir, fileName);
      const destination = path.join(stagingDir, fileName);
      if (!fs.existsSync(source)) {
        throw new Error(
          `Missing ${fileName} in ${inputDir}. Build the release binaries first before packaging the bundle.`,
        );
      }
      fs.copyFileSync(source, destination);
      if (!target.windows) {
        fs.chmodSync(destination, 0o755);
      }
    }

    const tar = spawnSync("tar", ["-czf", outputPath, "-C", tempRoot, path.basename(stagingDir)], {
      encoding: "utf8",
    });
    if (tar.status !== 0) {
      throw new Error(tar.stderr.trim() || "Failed to create release bundle.");
    }

    console.log(`Created ${outputPath}`);
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
}

function parseArgs(argv) {
  const args = {};
  for (let index = 0; index < argv.length; index += 1) {
    const current = argv[index];
    if (current === "--help" || current === "-h") {
      args.help = true;
      continue;
    }
    if (!current.startsWith("--")) {
      continue;
    }
    const key = current.slice(2);
    const value = argv[index + 1];
    if (!value || value.startsWith("--")) {
      args[key] = true;
      continue;
    }
    args[key] = value;
    index += 1;
  }
  return args;
}

function printHelp(error) {
  if (error) {
    console.error(error);
    console.error("");
  }
  console.error("Usage:");
  console.error(
    "  node scripts/package-release-bundle.cjs --platform <darwin|linux|windows> --arch <x64|arm64> [--input-dir target/release] [--output-dir dist/release-bundles] [--version 0.1.0]",
  );
}

function resolveTarget(platform, arch) {
  if (!["darwin", "linux", "windows"].includes(platform)) {
    throw new Error(`Unsupported --platform value: ${platform}`);
  }
  if (!["x64", "arm64"].includes(arch)) {
    throw new Error(`Unsupported --arch value: ${arch}`);
  }
  return {
    platform,
    arch,
    windows: platform === "windows",
  };
}

function bundleBinaryNames(target) {
  const suffix = target.windows ? ".exe" : "";
  return [
    `relora${suffix}`,
    `relora-driver-postgres${suffix}`,
    `relora-driver-mysql${suffix}`,
    `relora-driver-sqlite${suffix}`,
  ];
}

function ensureTarAvailable() {
  const check = spawnSync("tar", ["--version"], { stdio: "ignore" });
  if (check.error || check.status !== 0) {
    throw new Error("The `tar` command is required to build a Relora release bundle.");
  }
}

function readWorkspaceVersion() {
  const cargoToml = fs.readFileSync(path.join(workspaceRoot, "Cargo.toml"), "utf8");
  const match = cargoToml.match(/\[workspace\.package\][\s\S]*?version\s*=\s*"([^"]+)"/);
  if (!match) {
    throw new Error("Unable to read workspace version from Cargo.toml.");
  }
  return match[1];
}
