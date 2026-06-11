# Frontend Forge

Frontend Forge builds KubeSphere frontend extension manifests into SystemJS bundles.

The production path is a Rust CLI/K8s Job. The previous HTTP server API has been
removed; inputs and outputs are file based.

## Project Structure

- `crates/forge-job`: Rust CLI entrypoint for build jobs, file IO, Rollup execution, validation, and archive output.
- `crates/forge-core`: orchestration boundary that creates build plans without binding to Rollup, pnpm, or archive details.
- `crates/forge-component-generator`: Rust component tree to TSX generator, Oxc by default and SWC behind the `swc` feature.
- `crates/forge-project-generator`: manifest to plain Rollup project files.
- `packages/forge-components`: TS/React runtime component package used by generated pages.
- `examples/`: local manifest and page-schema examples for CLI testing.

## Build Job

Main command:

```bash
frontend-forge-job build --input /input/manifest.json --out-dir /output
```

Outputs:

- `/output/build.tar.gz`: Rollup-built SystemJS files.
- `/output/result.json`: build status, timings, versions, warnings, and errors.
- `/output/project.tar.gz`: optional generated project source archive.

`project.tar.gz` is disabled by default and can be enabled with:

```bash
frontend-forge-job build \
  --input /input/manifest.json \
  --out-dir /output \
  --emit-project-archive true
```

The environment variable `FORGE_EMIT_PROJECT_ARCHIVE=true|false` is also
supported. CLI flags take precedence over environment variables.

## Local Development

Install dependencies:

```bash
pnpm install
```

The pnpm workspace is intentionally narrow: `crates/forge-job` provides the
Rollup/Node runtime dependencies, and `packages/forge-components` is the
TS/React runtime package linked by the Job.

Build the TS runtime components when validating the published package output:

```bash
pnpm --filter @frontend-forge/forge-components build
```

Dev-mode Job builds (`FORGE_DEV_MODE=true`) resolve
`packages/forge-components/src/index.ts` when the workspace-linked package has a
source entry, so this build step is not required before every local manifest
build. Production builds keep using the package default entry.

Run the Rust tests:

```bash
cargo test --workspace
cargo test --workspace --features swc
```

Run a local full build with the sample manifest:

```bash
FORGE_NODE_MODULES_DIR=/Users/yazhou/code/frontend-forge/crates/forge-job/node_modules \
FORGE_ROLLUP_BIN=/Users/yazhou/code/frontend-forge/crates/forge-job/node_modules/.bin/rollup \
FORGE_DEV_MODE=true \
cargo run -p forge-job --bin frontend-forge-job -- build \
  --input examples/full.json \
  --out-dir .tmp/full-flow/local-out \
  --emit-project-archive true
```

Inspect the bundle header:

```bash
tar -xOf .tmp/full-flow/local-out/build.tar.gz index.js | sed -n '1,12p'
```

## Docker

Build the Job image:

```bash
NPM_AUTH_BASE64='***' docker build \
  -f crates/forge-job/Dockerfile \
  --target prod \
  --secret id=npm_auth_base64,env=NPM_AUTH_BASE64 \
  -t frontend-forge-job:prod .
```

Build the shell-friendly debug image:

```bash
NPM_AUTH_BASE64='***' docker build \
  -f crates/forge-job/Dockerfile \
  --target dev \
  --secret id=npm_auth_base64,env=NPM_AUTH_BASE64 \
  -t frontend-forge-job:dev .
```

Run a smoke test:

```bash
rm -rf .tmp/full-flow/docker-input .tmp/full-flow/docker-out
mkdir -p .tmp/full-flow/docker-input .tmp/full-flow/docker-out
cp examples/full.json .tmp/full-flow/docker-input/manifest.json

docker run --rm \
  -v /Users/yazhou/code/frontend-forge/.tmp/full-flow/docker-input:/input:ro \
  -v /Users/yazhou/code/frontend-forge/.tmp/full-flow/docker-out:/output \
  frontend-forge-job:prod \
  build --input /input/manifest.json --out-dir /output --emit-project-archive true
```

## Notes

- Temporary output is written under `.tmp/`; do not commit generated files.
- Generated SystemJS bundles should contain `System.register` and must not contain webpack runtime tokens or executable dynamic `import(`.
- Manifest expression/code inputs are trusted inputs for the build job, not a sandbox for untrusted code.
