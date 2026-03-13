# syntax=docker/dockerfile:1

FROM node:22-bookworm-slim AS deps
WORKDIR /app

RUN corepack enable

COPY package.json pnpm-lock.yaml pnpm-workspace.yaml .npmrc ./
COPY apps/server/package.json apps/server/
COPY packages/vendor/package.json packages/vendor/
COPY packages/code-export/package.json packages/code-export/
COPY packages/component-generator/package.json packages/component-generator/
COPY packages/forge-components/package.json packages/forge-components/
COPY packages/forge-core/package.json packages/forge-core/
COPY packages/project-generator/package.json packages/project-generator/

RUN pnpm i --frozen-lockfile --filter @frontend-forge/server... --filter @frontend-forge/vendor...

FROM node:22-bookworm-slim AS build
WORKDIR /app

RUN corepack enable

COPY --from=deps /app/node_modules ./node_modules
COPY . .
COPY --from=deps /app/apps/server/node_modules ./apps/server/node_modules
COPY --from=deps /app/packages/code-export/node_modules ./packages/code-export/node_modules
COPY --from=deps /app/packages/component-generator/node_modules ./packages/component-generator/node_modules
COPY --from=deps /app/packages/forge-components/node_modules ./packages/forge-components/node_modules
COPY --from=deps /app/packages/forge-core/node_modules ./packages/forge-core/node_modules
COPY --from=deps /app/packages/project-generator/node_modules ./packages/project-generator/node_modules

RUN pnpm --filter @frontend-forge/forge-components build \
 && pnpm -r --filter @frontend-forge/server... run build
RUN pnpm --filter @frontend-forge/server deploy --prod /app/server
RUN pnpm --filter @frontend-forge/vendor deploy --prod /app/server/vendor
RUN find /app/server -path '*/node_modules/*' -type d \( \
    -name test -o -name tests -o -name __tests__ -o -name docs -o -name doc -o -name examples -o -name example -o -name benchmark -o -name benchmarks \
  \) -prune -exec rm -rf '{}' + \
  && find /app/server -path '*/node_modules/*' -type f \( \
    -name '*.md' -o -name '*.markdown' -o -name '*.map' \
  \) -delete \
  && mkdir -p /data/cache \
  && chown -R 65532:65532 /data
  
RUN rm -rf /app/server/node_modules/.pnpm/*musl* \
 && rm -rf /app/server/vendor/node_modules/.pnpm/*musl*

FROM gcr.io/distroless/nodejs22-debian12 AS runner
WORKDIR /app/server
ENV NODE_ENV=production

COPY --from=build /app/server ./
COPY --from=build /data /data

ENV CACHE_DIR=/data/cache
USER 65532:65532
EXPOSE 3000

CMD ["dist/server.js"]
