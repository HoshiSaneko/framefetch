import { existsSync } from "node:fs";
import { delimiter, dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { spawn } from "node:child_process";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const env = { ...process.env };
const cargo = resolve(root, ".tools/cargo");
const rustup = resolve(root, ".tools/rustup");
if (existsSync(resolve(cargo, "bin", process.platform === "win32" ? "cargo.exe" : "cargo"))) {
  env.CARGO_HOME = cargo;
  env.RUSTUP_HOME = rustup;
  env.PATH = `${resolve(cargo, "bin")}${delimiter}${env.PATH ?? ""}`;
}
const child = spawn(process.execPath, [resolve(root, "node_modules/@tauri-apps/cli/tauri.js"), ...process.argv.slice(2)], {
  cwd: root, env, stdio: "inherit",
});
child.on("error", (error) => { console.error(error); process.exitCode = 1; });
child.on("exit", (code) => { process.exitCode = code ?? 1; });
