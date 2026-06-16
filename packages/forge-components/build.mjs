import { build } from "tsdown";
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
  const isExternalPackage = (id) =>
    externalPackages.some((name) => id === name || id.startsWith(`${name}/`));

  await build({
    config: false,
    entry: ["src/index.ts"],
    outDir: "dist",
    format: "esm",
    platform: "browser",
    clean: false,
    dts: false,
    fixedExtension: false,
    logLevel: "error",
    report: false,
    sourcemap: true,
    tsconfig: false,
    deps: {
      neverBundle: isExternalPackage,
      alwaysBundle: (id) => !isExternalPackage(id),
      onlyBundle: false,
    },
    inputOptions: {
      transform: {
        jsx: "react-jsx",
      },
    },
    outputOptions: {
      entryFileNames: "index.js",
    },
  });

  await runCommand("tsc", ["-p", "tsconfig.json"]);
}

runBuild().catch((err) => {
  console.error(err);
  process.exitCode = 1;
});
