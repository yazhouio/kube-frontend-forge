# syntax=docker/dockerfile:1.7

FROM rustlang/rust:nightly-bookworm AS rust-builder

WORKDIR /workspace
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY schemas ./schemas
COPY xtask ./xtask
RUN --mount=type=cache,id=forge-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=forge-cargo-git,target=/usr/local/cargo/git \
    cargo build --release -p forge-job

FROM node:22-bookworm-slim AS node-deps

ENV PNPM_HOME="/pnpm"
ENV PATH="$PNPM_HOME:$PATH"
RUN corepack enable && corepack prepare pnpm@11.1.0 --activate

WORKDIR /workspace
COPY .npmrc package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY crates/forge-job/package.json ./crates/forge-job/package.json
COPY packages/forge-components/package.json ./packages/forge-components/package.json
RUN --mount=type=cache,id=forge-pnpm-store,target=/pnpm/store \
    --mount=type=secret,id=npm_auth_base64 \
    set -eu; \
    cleanup() { \
      pnpm config delete "//internal-registry.npm.kubesphere.com.cn/:_auth" >/dev/null 2>&1 || true; \
      pnpm config delete "//internal-registry.npm.kubesphere.com.cn/:always-auth" >/dev/null 2>&1 || true; \
    }; \
    trap cleanup EXIT; \
    if [ -s /run/secrets/npm_auth_base64 ]; then \
      NPM_AUTH_BASE64="$(cat /run/secrets/npm_auth_base64)"; \
      pnpm config set "//internal-registry.npm.kubesphere.com.cn/:_auth" "$NPM_AUTH_BASE64"; \
      pnpm config set "//internal-registry.npm.kubesphere.com.cn/:always-auth" true; \
    fi; \
    pnpm install --frozen-lockfile --ignore-scripts --store-dir /pnpm/store \
      --filter @frontend-forge/forge-components \
      --filter @frontend-forge/forge-job-runtime

COPY packages/forge-components ./packages/forge-components
RUN pnpm --filter @frontend-forge/forge-components build
RUN --mount=type=cache,id=forge-pnpm-store,target=/pnpm/store \
    pnpm --store-dir /pnpm/store --config.inject-workspace-packages=true --filter @frontend-forge/forge-job-runtime deploy --prod /runtime/forge-job \
    && find /runtime/forge-job -path '*/node_modules/*' -type d \( \
      -name test -o -name tests -o -name __tests__ -o -name docs -o -name doc -o -name examples -o -name example -o -name benchmark -o -name benchmarks \
    \) -prune -exec rm -rf '{}' + \
    && find /runtime/forge-job -path '*/node_modules/*' -type f \( \
      -name '*.md' -o -name '*.markdown' -o -name '*.map' \
    \) -delete \
    && rm -rf /runtime/forge-job/node_modules/.pnpm/*darwin* \
    && rm -rf /runtime/forge-job/node_modules/.pnpm/*win32* \
    && rm -rf /runtime/forge-job/node_modules/.pnpm/*musl*

FROM node:22-bookworm-slim AS dev

ENV NODE_ENV="production"
ENV FORGE_DEV_MODE="true"
ENV FORGE_NODE_MODULES_DIR="/app/node_modules"
ENV FORGE_COMPONENTS_PACKAGE_JSON="/app/node_modules/@frontend-forge/forge-components/package.json"
ENV FORGE_PNPM_VERSION="11.1.0"
ENV FORGE_NODE_VERSION="22.22.3"

WORKDIR /app
COPY --from=rust-builder /workspace/target/release/frontend-forge-job /usr/local/bin/frontend-forge-job
COPY --from=node-deps /runtime/forge-job/node_modules ./node_modules

ENTRYPOINT ["frontend-forge-job"]
CMD ["build", "--input", "/input/manifest.json", "--out-dir", "/output"]

FROM node:22-bookworm-slim AS server-dev

ENV NODE_ENV="production"
ENV FORGE_DEV_MODE="true"
ENV FORGE_NODE_MODULES_DIR="/app/node_modules"
ENV FORGE_COMPONENTS_PACKAGE_JSON="/app/node_modules/@frontend-forge/forge-components/package.json"
ENV FORGE_PNPM_VERSION="11.1.0"
ENV FORGE_NODE_VERSION="22.22.3"
ENV RUST_LOG="frontend_forge_server=debug,tower_http=debug,axum=debug,forge_project_generator=debug,forge_component_generator=debug"

WORKDIR /app
COPY --from=rust-builder /workspace/target/release/frontend-forge-server /usr/local/bin/frontend-forge-server
COPY --from=node-deps /runtime/forge-job/node_modules ./node_modules

EXPOSE 3000
ENTRYPOINT ["frontend-forge-server"]
CMD ["--addr", "0.0.0.0:3000"]

FROM gcr.io/distroless/cc-debian12 AS prod

ENV NODE_ENV="production"
ENV FORGE_NODE_MODULES_DIR="/app/node_modules"
ENV FORGE_COMPONENTS_PACKAGE_JSON="/app/node_modules/@frontend-forge/forge-components/package.json"
ENV FORGE_PNPM_VERSION="11.1.0"

WORKDIR /app
COPY --from=rust-builder /workspace/target/release/frontend-forge-job /usr/local/bin/frontend-forge-job
COPY --from=node-deps /runtime/forge-job/node_modules ./node_modules

ENTRYPOINT ["/usr/local/bin/frontend-forge-job"]
CMD ["build", "--input", "/input/manifest.json", "--out-dir", "/output"]

FROM gcr.io/distroless/cc-debian12 AS server-prod

ENV NODE_ENV="production"
ENV FORGE_NODE_MODULES_DIR="/app/node_modules"
ENV FORGE_COMPONENTS_PACKAGE_JSON="/app/node_modules/@frontend-forge/forge-components/package.json"
ENV FORGE_PNPM_VERSION="11.1.0"

WORKDIR /app
COPY --from=rust-builder /workspace/target/release/frontend-forge-server /usr/local/bin/frontend-forge-server
COPY --from=node-deps /runtime/forge-job/node_modules ./node_modules

EXPOSE 3000
ENTRYPOINT ["/usr/local/bin/frontend-forge-server"]
CMD ["--addr", "0.0.0.0:3000"]
