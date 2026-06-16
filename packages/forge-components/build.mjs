import { build } from "rolldown";
import { spawn } from "node:child_process";

const externalPackages = [
  "react",
  "react-dom",
  "react-router-dom",
  "react/jsx-runtime",
  "react/jsx-dev-runtime",
  "swr",
  "@kubed/components",
  "@kubed/hooks",
  "@kubed/code-editor",
  "@kubed/icons",
  "zustand",
  "styled-components",
  "esprima",
];

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
    input: "src/index.ts",
    platform: "browser",
    transform: {
      jsx: "react-jsx",
    },
    external: (id) =>
      externalPackages.some((name) => id === name || id.startsWith(`${name}/`)),
    output: {
      dir: "dist",
      entryFileNames: "index.js",
      format: "esm",
      sourcemap: true,
    },
  });

  await runCommand("tsc", ["-p", "tsconfig.json"]);
}

runBuild().catch((err) => {
  console.error(err);
  process.exitCode = 1;
});
