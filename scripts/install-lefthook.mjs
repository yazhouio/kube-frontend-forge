import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";

const hasGit = spawnSync("git", ["--version"], { stdio: "ignore" }).status === 0;

if (!hasGit || !existsSync(".git")) {
  process.exit(0);
}

const result = spawnSync("lefthook", ["install"], { stdio: "inherit" });
process.exit(result.status ?? 1);
