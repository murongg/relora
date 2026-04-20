#!/usr/bin/env node

const fs = require("node:fs");
const fsp = require("node:fs/promises");
const https = require("node:https");
const os = require("node:os");
const path = require("node:path");
const { pipeline } = require("node:stream/promises");
const { spawnSync } = require("node:child_process");

const packageJson = require("../package.json");

const packageRoot = path.resolve(__dirname, "..");
const distDir = path.join(packageRoot, "dist");
const binDir = path.join(distDir, "bin");
const installMarker = path.join(binDir, ".relora-install.json");

main().catch((error) => {
  console.error(`Relora install failed: ${error.message}`);
  process.exit(1);
});

async function main() {
  if (process.env.RELORA_NPM_SKIP_POSTINSTALL === "1") {
    console.log("Skipping Relora postinstall because RELORA_NPM_SKIP_POSTINSTALL=1.");
    return;
  }

  const target = resolveTarget(process.platform, process.arch);
  const assetName = releaseAssetName(packageJson.version, target);
  const baseUrl =
    process.env.RELORA_NPM_BASE_URL ??
    `https://github.com/murongg/relora/releases/download/v${packageJson.version}`;
  const downloadUrl = `${baseUrl}/${assetName}`;
  const executableName = target.windows ? "relora.exe" : "relora";
  const executablePath = path.join(binDir, executableName);

  if (fs.existsSync(executablePath) && fs.existsSync(installMarker)) {
    return;
  }

  ensureTarAvailable();

  fs.rmSync(distDir, { recursive: true, force: true });
  fs.mkdirSync(binDir, { recursive: true });

  const tempRoot = fs.mkdtempSync(path.join(os.tmpdir(), "relora-npm-"));
  const archivePath = path.join(tempRoot, assetName);
  const extractDir = path.join(tempRoot, "extract");
  fs.mkdirSync(extractDir, { recursive: true });

  try {
    console.log(`Downloading Relora ${packageJson.version} for ${target.label}...`);
    await download(downloadUrl, archivePath);
    extractArchive(archivePath, extractDir);

    const bundleRoot = resolveBundleRoot(extractDir, target);
    copyBundleFiles(bundleRoot, binDir, target);

    await fsp.writeFile(
      installMarker,
      JSON.stringify(
        {
          version: packageJson.version,
          target: target.label,
          assetName,
          downloadedFrom: downloadUrl,
        },
        null,
        2,
      ),
      "utf8",
    );

    console.log("Relora runtime installed successfully.");
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
}

function resolveTarget(platform, arch) {
  const supportedArch = arch === "x64" || arch === "arm64";
  if (!supportedArch) {
    throw new Error(
      `Unsupported architecture: ${arch}. Relora npm currently supports x64 and arm64.`,
    );
  }

  switch (platform) {
    case "darwin":
      return { platform: "darwin", arch, label: `darwin-${arch}`, windows: false };
    case "linux":
      return { platform: "linux", arch, label: `linux-${arch}`, windows: false };
    case "win32":
      return { platform: "windows", arch, label: `windows-${arch}`, windows: true };
    default:
      throw new Error(
        `Unsupported platform: ${platform}. Relora npm currently supports macOS, Linux, and Windows.`,
      );
  }
}

function releaseAssetName(version, target) {
  return `relora-v${version}-${target.platform}-${target.arch}.tar.gz`;
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
    throw new Error(
      "The `tar` command is required to unpack the Relora runtime bundle. Install tar or use the manual release bundle.",
    );
  }
}

async function download(url, destination) {
  const output = fs.createWriteStream(destination);
  try {
    await request(url, 0, output);
  } finally {
    output.close();
  }
}

async function request(url, redirects, output) {
  if (redirects > 5) {
    throw new Error(`Too many redirects while downloading ${url}`);
  }

  await new Promise((resolve, reject) => {
    const request = https.get(
      url,
      {
        headers: {
          "User-Agent": "relora npm installer",
          Accept: "application/octet-stream",
        },
      },
      async (response) => {
        try {
          const status = response.statusCode ?? 0;
          if (
            status >= 300 &&
            status < 400 &&
            typeof response.headers.location === "string"
          ) {
            response.resume();
            await request(new URL(response.headers.location, url).toString(), redirects + 1, output);
            resolve();
            return;
          }

          if (status !== 200) {
            response.resume();
            reject(
              new Error(
                `Unexpected HTTP ${status} while downloading ${url}. Make sure the GitHub release contains the matching runtime bundle.`,
              ),
            );
            return;
          }

          await pipeline(response, output);
          resolve();
        } catch (error) {
          reject(error);
        }
      },
    );

    request.on("error", reject);
  });
}

function extractArchive(archivePath, extractDir) {
  const result = spawnSync("tar", ["-xzf", archivePath, "-C", extractDir], {
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(result.stderr.trim() || "Unable to extract Relora runtime bundle.");
  }
}

function resolveBundleRoot(extractDir, target) {
  const expected = new Set(bundleBinaryNames(target));
  const directEntries = fs.readdirSync(extractDir);
  if (directEntries.some((entry) => expected.has(entry))) {
    return extractDir;
  }

  if (directEntries.length === 1) {
    const nested = path.join(extractDir, directEntries[0]);
    if (fs.existsSync(nested) && fs.statSync(nested).isDirectory()) {
      const nestedEntries = fs.readdirSync(nested);
      if (nestedEntries.some((entry) => expected.has(entry))) {
        return nested;
      }
    }
  }

  throw new Error("Downloaded bundle did not contain the expected Relora binaries.");
}

function copyBundleFiles(bundleRoot, destinationDir, target) {
  for (const fileName of bundleBinaryNames(target)) {
    const source = path.join(bundleRoot, fileName);
    const destination = path.join(destinationDir, fileName);
    if (!fs.existsSync(source)) {
      throw new Error(`Downloaded bundle is missing ${fileName}.`);
    }
    fs.copyFileSync(source, destination);
    if (!target.windows) {
      fs.chmodSync(destination, 0o755);
    }
  }
}
