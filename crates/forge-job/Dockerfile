# syntax=docker/dockerfile:1.7

FROM rust:1-bookworm AS rust-builder

WORKDIR /workspace
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release -p forge-job

FROM node:22-bookworm-slim AS node-deps

ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
RUN corepack enable && corepack prepare pnpm@11.1.0 --activate

WORKDIR /workspace
COPY .npmrc package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY crates/forge-job/package.json ./crates/forge-job/package.json
COPY packages/forge-components/package.json ./packages/forge-components/package.json
RUN --mount=type=secret,id=npm_auth_base64 \
    set -eu; \
    cleanup() { \
      pnpm config delete "//internal-registry.npm.kubesphere.com.cn/:_auth" >/dev/null 2>&1 || true; \
      pnpm config delete "//internal-registry.npm.kubesphere.com.cn/:always-auth" >/dev/null 2>&1 || true; \
    }; \
    trap cleanup EXIT; \
    if [ -f /run/secrets/npm_auth_base64 ]; then \
      NPM_AUTH_BASE64="$(cat /run/secrets/npm_auth_base64)"; \
      pnpm config set "//internal-registry.npm.kubesphere.com.cn/:_auth" "$NPM_AUTH_BASE64"; \
      pnpm config set "//internal-registry.npm.kubesphere.com.cn/:always-auth" true; \
    fi; \
    pnpm install --frozen-lockfile

COPY packages/forge-components ./packages/forge-components
RUN pnpm --filter @frontend-forge/forge-components build

FROM node:22-bookworm-slim AS runtime

ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
ENV FORGE_NODE_MODULES_DIR="/app/crates/forge-job/node_modules"
ENV FORGE_ROLLUP_BIN="/app/crates/forge-job/node_modules/.bin/rollup"
ENV FORGE_COMPONENTS_PACKAGE_JSON="/app/packages/forge-components/package.json"

RUN corepack enable && corepack prepare pnpm@11.1.0 --activate

WORKDIR /app
COPY --from=rust-builder /workspace/target/release/frontend-forge-job /usr/local/bin/frontend-forge-job
COPY --from=node-deps /workspace/node_modules ./node_modules
COPY --from=node-deps /workspace/crates/forge-job/node_modules ./crates/forge-job/node_modules
COPY --from=node-deps /workspace/packages/forge-components ./packages/forge-components

ENTRYPOINT ["frontend-forge-job"]
CMD ["build", "--input", "/input/manifest.json", "--out-dir", "/output"]
