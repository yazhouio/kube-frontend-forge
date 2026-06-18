# Frontend Forge

Frontend Forge builds KubeSphere frontend extension manifests into SystemJS bundles.

The production path is a Rust CLI/K8s Job. The previous HTTP server API has been
removed; inputs and outputs are file based.

## Project Structure

- `crates/forge-job`: Rust CLI entrypoint for build jobs, file IO, project build execution, validation, and archive output.
- `crates/forge-core`: orchestration boundary that creates build plans without binding to a bundler, pnpm, or archive details.
- `crates/forge-component-generator`: Rust component tree to TSX generator, Oxc by default and SWC behind the `swc` feature.
- `crates/forge-project-generator`: manifest to generated project files.
- `packages/forge-components`: TS/React runtime component package used by generated pages.
- `examples/`: local manifest and page-schema examples for CLI testing.

## Build Job

Main command:

```bash
frontend-forge-job build --input /input/manifest.json --out-dir /output
```

Outputs:

- `/output/build.tar.gz`: built output files, SystemJS by default.
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

### Build Engine

The Job uses the in-process rspack backend by default through the `rspack-build`
feature. `FORGE_BUILD_ENGINE=rolldown` selects the optional Rust Rolldown
backend; build `forge-job` with `--features rolldown-build --no-default-features`
or combine `rspack-build,rolldown-build` when both engines are needed. The
aliases `rolldown-rust`, `rulldown`, and `rulldown-rust` are accepted for the
same backend.

Rolldown output is first bundled as ESM. For SystemJS manifests, the Job then
uses SWC to convert every emitted JavaScript file to `System.register` before
archive validation. The CLI also keeps the legacy generated `build.mjs` path
available through `FORGE_BUILD_ENGINE=node|build.mjs|esbuild`; the HTTP server
supports the in-process rspack and rolldown backends.

## Local Development

Install dependencies:

```bash
pnpm install
```

The pnpm workspace is intentionally narrow: `crates/forge-job` provides the
Node runtime dependencies for the legacy build script, and
`packages/forge-components` is the TS/React runtime package linked by the Job.

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

For faster local Rust linking, Linux developers are recommended to use `mold`
and macOS developers are recommended to use `sold` (`ld64.sold`). Add this as
machine-local Cargo config only, so machines without the linker still build
normally.

Linux:

```toml
# ~/.cargo/config.toml
[target.x86_64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]

[target.aarch64-unknown-linux-gnu]
linker = "clang"
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
```

macOS:

```toml
# ~/.cargo/config.toml
[target.aarch64-apple-darwin]
rustflags = ["-C", "link-arg=-Wl,--ld-path=/path/to/ld64.sold"]

[target.x86_64-apple-darwin]
rustflags = ["-C", "link-arg=-Wl,--ld-path=/path/to/ld64.sold"]
```

Run a local full build with the sample manifest:

```bash
FORGE_DEV_MODE=true \
cargo run -p forge-job --bin frontend-forge-job -- build \
  --input examples/full.json \
  --out-dir .tmp/full-flow/local-out \
  --emit-project-archive true
```

Run the same sample with the optional Rolldown backend:

```bash
FORGE_BUILD_ENGINE=rolldown \
cargo run -p forge-job --features rolldown-build --no-default-features \
  --bin frontend-forge-job -- build \
  --input examples/full.json \
  --out-dir .tmp/full-flow/rolldown-out \
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
  --target job-prod \
  --secret id=npm_auth_base64,env=NPM_AUTH_BASE64 \
  -t frontend-forge-job:prod .
```

Build the shell-friendly debug image:

```bash
NPM_AUTH_BASE64='***' docker build \
  -f crates/forge-job/Dockerfile \
  --target job-debug \
  --secret id=npm_auth_base64,env=NPM_AUTH_BASE64 \
  -t frontend-forge-job:debug .
```

Build the HTTP server image:

```bash
NPM_AUTH_BASE64='***' docker build \
  -f crates/forge-job/Dockerfile \
  --target server-prod \
  --secret id=npm_auth_base64,env=NPM_AUTH_BASE64 \
  -t frontend-forge-server:prod .
```

Run the HTTP server with TOML config:

```bash
docker run --rm -p 3000:3000 \
  -v "$(pwd)/examples/server.toml:/config/server.toml:ro" \
  frontend-forge-server:prod \
  --config /config/server.toml
```

Run a smoke test:

```bash
rm -rf .tmp/full-flow/docker-input .tmp/full-flow/docker-out
mkdir -p .tmp/full-flow/docker-input .tmp/full-flow/docker-out
cp examples/full.json .tmp/full-flow/docker-input/manifest.json

docker run --rm \
  -v "$(pwd)/.tmp/full-flow/docker-input:/input:ro" \
  -v "$(pwd)/.tmp/full-flow/docker-out:/output" \
  frontend-forge-job:prod \
  build --input /input/manifest.json --out-dir /output --emit-project-archive true
```

## Notes

- Temporary output is written under `.tmp/`; do not commit generated files.
- Generated SystemJS bundles should contain `System.register` and must not contain webpack runtime tokens or executable dynamic `import(`.
- Manifest expression/code inputs are trusted inputs for the build job, not a sandbox for untrusted code.
