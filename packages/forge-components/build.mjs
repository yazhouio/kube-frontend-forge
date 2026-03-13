import { build } from "esbuild";
import { spawn } from "node:child_process";

function runCommand(command, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { stdio: "inherit" });
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) resolve();
      else
        reject(
          new Error(
            `${command} ${args.join(" ")} exited with code ${code ?? "unknown"}`,
          ),
        );
    });
  });
}

async function runBuild() {
  await build({
    entryPoints: ["src/index.ts"],
    bundle: true,
    outdir: "dist",
    format: "esm",
    platform: "browser",
    jsx: "automatic",
    external: [
      "react",
      "react-dom",
      "react-router-dom",
      "react/jsx-runtime",
      "react/jsx-dev-runtime",
      "swr",
      "swr/*",
      "@kubed/components",
      "@kubed/hooks",
      "@kubed/code-editor",
      "@kubed/icons",
      "zustand",
      "zustand/*",
      "styled-components",
    ],
    sourcemap: true,
  });

  await runCommand("tsc", ["-p", "tsconfig.json"]);
}

runBuild().catch((err) => {
  console.error(err);
  process.exitCode = 1;
});
