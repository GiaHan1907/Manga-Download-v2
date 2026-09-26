import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

// P10-02 version invariant: ONE authoritative version reflected in npm,
// Tauri and Rust metadata. The release pipeline fails loudly on drift.
const root = fileURLToPath(new URL("..", import.meta.url));
const read = (p) => readFileSync(root + p, "utf8");

const pkg = JSON.parse(read("package.json")).version;
const tauri = JSON.parse(read("src-tauri/tauri.conf.json")).version;
const cargo = read("src-tauri/Cargo.toml").match(/^version = "([^"]+)"/m)?.[1];
const lock = read("src-tauri/Cargo.lock").match(
  /name = "manga-downloader-1989"\r?\nversion = "([^"]+)"/,
)?.[1];

const entries = { "package.json": pkg, "tauri.conf.json": tauri, "Cargo.toml": cargo, "Cargo.lock": lock };
const unique = new Set(Object.values(entries));
for (const [name, value] of Object.entries(entries)) {
  console.log(`${name}: ${value ?? "MISSING"}`);
}
if (unique.size !== 1 || unique.has(undefined)) {
  console.error("VERSION DRIFT — release gate failed");
  process.exit(1);
}
if (!/^\d+\.\d+\.\d+(-[\w.]+)?$/.test(pkg)) {
  console.error(`version "${pkg}" is not valid SemVer`);
  process.exit(1);
}
console.log(`OK — single version ${pkg}`);
