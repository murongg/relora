#!/usr/bin/env node

const fs = require("node:fs");
const path = require("node:path");

const repoRoot = path.resolve(__dirname, "..");
const rootPackagePath = path.join(repoRoot, "package.json");
const npmPackageRelativePath = "packages/relora-npm/package.json";
const cargoTomlRelativePath = "Cargo.toml";
const cargoLockRelativePath = "Cargo.lock";
const npmPackagePath = path.join(repoRoot, "packages", "relora-npm", "package.json");
const cargoTomlPath = path.join(repoRoot, "Cargo.toml");
const cargoLockPath = path.join(repoRoot, "Cargo.lock");
const workspacePackageNames = new Set([
  "relora",
  "relora-app",
  "relora-core",
  "relora-driver-postgres",
  "relora-driver-mysql",
  "relora-driver-sqlite",
]);

main();

function main() {
  const rootPackage = JSON.parse(fs.readFileSync(rootPackagePath, "utf8"));
  const version = rootPackage.version;
  if (!version || typeof version !== "string") {
    throw new Error("Root package.json does not contain a valid version string.");
  }

  syncNpmPackageVersion(version);
  syncCargoWorkspaceVersion(version);
  syncCargoLockVersions(version);

  console.log(`Synchronized Relora workspace version to ${version}.`);
}

function syncNpmPackageVersion(version) {
  const source = JSON.parse(fs.readFileSync(npmPackagePath, "utf8"));
  source.version = version;
  fs.writeFileSync(npmPackagePath, `${JSON.stringify(source, null, 2)}\n`, "utf8");
}

function syncCargoWorkspaceVersion(version) {
  const source = fs.readFileSync(cargoTomlPath, "utf8");
  const lines = source.split("\n");
  let inWorkspacePackage = false;
  let updatedVersion = false;

  const updated = lines
    .map((line) => {
      if (line.trim() === "[workspace.package]") {
        inWorkspacePackage = true;
        return line;
      }

      if (inWorkspacePackage && line.startsWith("[")) {
        inWorkspacePackage = false;
      }

      if (inWorkspacePackage && /^\s*version\s*=/.test(line)) {
        updatedVersion = true;
        return line.replace(/version\s*=\s*"[^"]+"/, `version = "${version}"`);
      }

      return line;
    })
    .join("\n");

  if (!updatedVersion) {
    throw new Error(`Unable to find [workspace.package] version in ${cargoTomlRelativePath}.`);
  }

  if (!updated.includes("[workspace.package]")) {
    throw new Error(`${cargoTomlRelativePath} no longer contains the workspace.package section.`);
  }

  fs.writeFileSync(cargoTomlPath, updated, "utf8");
}

function syncCargoLockVersions(version) {
  const source = fs.readFileSync(cargoLockPath, "utf8");
  const lines = source.split("\n");
  let currentPackageName = null;
  let updatedVersionCount = 0;

  const updated = lines
    .map((line) => {
      if (line.trim() === "[[package]]") {
        currentPackageName = null;
        return line;
      }

      const nameMatch = line.match(/^name = "([^"]+)"$/);
      if (nameMatch) {
        currentPackageName = nameMatch[1];
        return line;
      }

      if (
        currentPackageName &&
        workspacePackageNames.has(currentPackageName) &&
        /^version = "[^"]+"$/.test(line)
      ) {
        updatedVersionCount += 1;
        return `version = "${version}"`;
      }

      return line;
    })
    .join("\n");

  if (updatedVersionCount !== workspacePackageNames.size) {
    throw new Error(
      `Expected to update ${workspacePackageNames.size} workspace package versions in ${cargoLockRelativePath}, updated ${updatedVersionCount}.`,
    );
  }

  fs.writeFileSync(cargoLockPath, updated, "utf8");
}
