# FrontendForge

KubeSphere v4 插件构建服务：接收 TS/TSX/CSS 源码，通过 esbuild + SWC 生成 **SystemJS** 单文件 JS，支持可选 Tailwind CSS，同时具备缓存、并发队列与超时控制。

## 能力概览
- POST `/build` 接收文件数组并返回 SystemJS 产物（必含 `System.register`）
- POST `/page/code` 生成单页代码（schema → TS/TSX string）
- POST `/project/files` 生成项目 TS 源码数组（VirtualFile[]）
- POST `/project/files.tar.gz` 打包项目 TS 源码数组（tar.gz）
- POST `/api/project/build` 编译项目并返回产物数组（VirtualFile[]）
- POST `/api/project/build.tar.gz` 打包编译产物数组（tar.gz）
- POST `/scene/project/files` 由 Scene 生成项目文件（VirtualFile[]）
- POST `/scene/project/files.tar.gz` 由 Scene 打包项目文件（tar.gz）
- POST `/scene/project/build` 由 Scene 构建并返回产物数组（VirtualFile[]）
- POST `/scene/project/build.tar.gz` 由 Scene 打包构建产物（tar.gz）
- POST `/k8s/jsbundles` 编译 manifest 并调用 K8s 创建 JSBundle
- POST `/k8s/jsbundles/scene` 由 Scene 构建并调用 K8s 创建 JSBundle
- CRUD `/kapis/frontend-forge.io/v1alpha1/frontendintegrations` 前端集成管理（通过 JSBundle 代理）
- 可选 Tailwind v4 输出 CSS，和 JS 构建解耦
- 内存 LRU + 磁盘 JSON 缓存，命中即回
- 并发队列、超时与隔离的临时工作目录，防止资源争用与路径穿越
- 预置 external 白名单（React 等），无 Webpack runtime 依赖
- 核心构建逻辑由 `@frontend-forge/forge-core` 提供，Server 仅做 HTTP 适配

## 快速开始
```bash
# 安装依赖（首选 pnpm）
pnpm install

# vendor 依赖（构建时需要）
pnpm --filter @frontend-forge/vendor deploy --prod apps/server/vendor

# 本地开发（热重载）
pnpm --filter @frontend-forge/server dev
```

服务默认监听 `0.0.0.0:3000`，健康检查 `GET /healthz`。

## 静态文件服务（@fastify/static）
服务启动时会读取配置文件 `config.json`（可用环境变量 `CONFIG_PATH` 覆盖路径），根据配置挂载静态目录。

示例 `config.json`：
```json
{
  "k8s": { "server": "https://kubernetes.default.svc", "tokenCookieName": "token" },
  "static": [
    {
      "root": "./public",
      "prefix": "/",
      "index": ["index.html"],
      "cacheControl": "public, max-age=300"
    }
  ]
}
```

字段说明：
- `k8s.server`：K8s API Server 地址（用于 `/k8s/jsbundles`）
- `k8s.tokenCookieName`：从 cookie 读取 token 的字段名（默认 `token`）
- `static[].root`：静态目录（相对路径以 `config.json` 所在目录为基准）
- `static[].prefix`：URL 前缀（建议以 `/` 开头且以 `/` 结尾；例如 `/assets/`）
- `static[].index`：目录默认首页（`string[]` 或 `false`）
- `static[].cacheControl`：可选，设置响应头 `Cache-Control`

## CLI 生成项目（调试）
服务内置一个简单 CLI，用于根据 manifest 生成项目文件：
```bash
pnpm --filter @frontend-forge/server project:debug -- \
  --manifest apps/server/examples/manifest.sample.json \
  --out .tmp/project-demo \
  --force
```

参数说明：
- `--manifest`：manifest 路径（默认 `apps/server/examples/manifest.sample.json`）
- `--out`：输出目录（默认 `.tmp/project-*`）
- `--force`：允许写入非空目录

## API
请求体支持两种形式：
- `/page/code` 接受 `pageSchema` 或直接传 schema
- `/project/*` 接受 `manifest` 或直接传 manifest
- `/scene/*` 接受 `scene` 或直接传 scene

JSON 接口成功响应包含 `ok: true`，失败为 `ok: false` 且带 `error` 信息。`*.tar.gz` 接口成功时返回二进制内容。

### `POST /build`
```json
{
  "files": [{ "path": "src/index.tsx", "content": "..." }],
  "entry": "src/index.tsx",
  "externals": ["react"],
  "tailwind": { "enabled": true, "input": "src/index.css", "config": "tailwind.config.js" }
}
```

响应（成功）：
```json
{
  "ok": true,
  "cacheHit": false,
  "key": "sha256...",
  "outputs": {
    "js": { "path": "index.js", "content": "System.register(...)" },
    "css": { "path": "style.css", "content": "..." }
  },
  "meta": { "buildMs": 123, "queuedMs": 130 }
}
```

说明：
- `cacheHit`：未命中为 `false`，命中为 `"memory"` 或 `"disk"`。
- `externals` 会覆盖默认白名单，默认列表见 `apps/server/src/config.ts`。
- `entry` 为空时会自动选择 `src/index.tsx` 或 `src/index.ts`。

示例：
```bash
curl -X POST http://localhost:3000/build \
  -H 'Content-Type: application/json' \
  --data-raw '{
    "entry": "src/index.tsx",
    "tailwind": {"enabled": true, "input": "src/index.css", "config": "tailwind.config.js"},
    "files": [
      {"path": "src/index.tsx", "content": "import \"./index.css\"; export default function App(){ console.log(\"hi\"); }"},
      {"path": "src/index.css", "content": "@import \"tailwindcss\";"},
      {"path": "tailwind.config.js", "content": "export default { content: [\"./src/**/*.{ts,tsx}\"] };"}
    ]
  }'
```

### `POST /page/code`
```json
{
  "pageSchema": { "meta": { "id": "page-1" }, "root": { "id": "root", "type": "Layout" }, "context": {} }
}
```

响应（成功）：
```json
{ "ok": true, "code": "export default function Page() { ... }" }
```

### `POST /project/files`
```json
{
  "manifest": { "version": "1.0", "name": "demo", "routes": [], "menus": [], "locales": [], "pages": [] }
}
```

响应（成功）：
```json
{ "ok": true, "files": [{ "path": "src/index.ts", "content": "..." }] }
```

### `POST /project/files.tar.gz`
- 请求体同 `/project/files`
- 响应为 `tar.gz` 二进制内容（`Content-Type: application/gzip`）

### `POST /api/project/build`
- 请求体同 `/project/files`
- 返回编译后的 `VirtualFile[]`（SystemJS JS + 可选 CSS）

### `POST /api/project/build.tar.gz`
- 请求体同 `/project/files`
- 响应为编译产物的 `tar.gz`

### `POST /scene/project/files`
```json
{
  "scene": {
    "type": "crdTable",
    "config": { "meta": { "id": "crd-table", "name": "CRD Table", "path": "/crd-table" }, "crd": {}, "scope": "namespace", "page": { "id": "page-id", "title": "Table", "authKey": "jobs" }, "columns": [] }
  }
}
```

响应（成功）：
```json
{ "ok": true, "files": [{ "path": "src/index.ts", "content": "..." }] }
```

### `POST /scene/project/files.tar.gz`
- 请求体同 `/scene/project/files`
- 响应为 `tar.gz` 二进制内容（`Content-Type: application/gzip`）

### `POST /scene/project/build`
- 请求体同 `/scene/project/files`
- 返回编译后的 `VirtualFile[]`

### `POST /scene/project/build.tar.gz`
- 请求体同 `/scene/project/files`
- 响应为编译产物的 `tar.gz`

### `POST /k8s/jsbundles`
请求体：
```json
{
  "params": { "name": "demo-frontend-js-bundle", "extensionName": "devops", "namespace": "kubesphere-system", "cluster": "host" },
  "manifest": { "version": "1.0", "name": "demo", "routes": [], "menus": [], "locales": [], "pages": [] }
}
```

### `POST /k8s/jsbundles/scene`
请求体：
```json
{
  "params": { "name": "demo-frontend-js-bundle", "extensionName": "devops", "namespace": "kubesphere-system", "cluster": "host" },
  "scene": {
    "type": "crdTable",
    "config": { "meta": { "id": "crd-table", "name": "CRD Table", "path": "/crd-table" }, "crd": {}, "scope": "namespace", "page": { "id": "page-id", "title": "Table", "authKey": "jobs" }, "columns": [] }
  }
}
```

说明：
- 需要在 `config.json` 配置 `k8s.server`。
- 从 cookie 读取 token（默认 cookie 名为 `token`），拼接为请求头 `Authorization: Bearer <token>`，并 POST 到 `.../apis/extensions.kubesphere.io/v1alpha1/jsbundles`。
- 创建的资源结构为：`spec: { row: { "index.js": "<base64(compiled js)>", "style.css": "<base64(compiled css)>" } }`，key 由编译产物的 `VirtualFile[].path` 决定。
- `params.name`：JSBundle 名称（写入 `metadata.name`）。
- `params.extensionName`：扩展名（写入 `metadata.labels["kubesphere.io/extension-ref"]`）。
- 可选 `params.namespace`：如果提供，则写入 `metadata.annotations["meta.helm.sh/release-namespace"]`（不写入 `metadata.namespace`）。
- 可选 `params.cluster`：如果提供，则请求路径会在前面加上 `/clusters/{cluster}`（例如 `.../clusters/{cluster}/apis/extensions.kubesphere.io/v1alpha1/...`）。
- 会把请求中的 `manifest` 以 JSON 字符串形式写入 `metadata.annotations["frontend-forge.io/manifest"]`。

### `GET /kapis/frontend-forge.io/v1alpha1/frontendintegrations`
查询参数（可选）：
- `enabled`: `true|false`
- `type`: `crd|iframe`
- `name`: 前端集成名称

说明：
- 实际数据存放在 `JSBundle`，服务通过 `/kapis/extensions.kubesphere.io/v1alpha1/jsbundles` 读写。
- 会在 `metadata.labels` 写入筛选字段：
  - `frontend-forge.io/resource=frontendintegration`
  - `frontend-forge.io/enabled=true|false`
  - `frontend-forge.io/type=crd|iframe`
  - `frontend-forge.io/name=<name>`
- 真实 CR 内容会写入 `metadata.annotations["frontend-forge.io/frontendintegration"]`。

### `POST /kapis/frontend-forge.io/v1alpha1/frontendintegrations`
请求体为 `FrontendIntegration` CR（JSON），服务会创建对应 `JSBundle`。

### `GET /kapis/frontend-forge.io/v1alpha1/frontendintegrations/:name`
读取单个 `FrontendIntegration`。

### `PUT /kapis/frontend-forge.io/v1alpha1/frontendintegrations/:name`
更新单个 `FrontendIntegration`，要求请求体 `metadata.name` 与路径一致。

### `DELETE /kapis/frontend-forge.io/v1alpha1/frontendintegrations/:name`
删除单个 `FrontendIntegration`。

## 环境变量
- `PORT`：HTTP 端口，默认 3000
- `CONFIG_PATH`：运行时配置文件路径，默认 `./config.json`
- `CACHE_DIR`：磁盘缓存目录，默认 `.cache`
- `CACHE_MAX_ITEMS`：内存缓存大小，默认 200
- `CONCURRENCY`：并发构建数，默认 `cpu/2`
- `BUILD_TIMEOUT_MS`：构建超时时间，默认 30000
- `MAX_BODY_BYTES`：请求体大小限制，默认 1 MiB
- `CHILD_MAX_OLD_SPACE_MB`：子进程 Node 最大内存，默认 512

## 缓存与队列
- 内存 LRU + 磁盘 JSON 缓存，磁盘缓存目录由 `CACHE_DIR` 控制。
- 并发构建通过 `p-queue` 控制，`CONCURRENCY` 决定队列并发度。

## 约束与校验
- 拒绝绝对路径与 `..`，限定扩展名：`ts|tsx|js|jsx|css|json`
- 构建产物必须含 `System.register`，并拒绝 `__webpack_require__`、`webpackChunk`、`import(`
- 单入口构建，无代码分割；external 需在白名单或请求中传入

## Docker
```bash
# From repo root
docker build -f apps/server/Dockerfile -t frontendforge .
docker run --rm -p 3000:3000 \
  -e CACHE_DIR=/data/cache \
  -v $(pwd)/.cache:/data/cache \
  frontendforge
```

生产部署建议：限制 CPU/内存、设置 `--pids-limit`，并将容器 rootfs 设为只读，挂载 tmpfs 至 `/tmp` 以获得更好的隔离。

## 示例与验证
- `apps/server/examples/build.request.json`：`/build` 请求示例
- `apps/server/examples/page.schema.json`：`/page/code` 请求示例
- `apps/server/examples/manifest.test.json`：`/project/*` 请求示例
- `apps/server/test.sh`：端到端接口验证脚本
