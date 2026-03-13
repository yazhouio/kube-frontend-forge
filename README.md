# FrontendForge

KubeSphere v4 插件构建服务与低代码构建链路。接收 TS/TSX/CSS 源码或页面/项目 manifest，输出 SystemJS 单文件产物，并提供缓存、并发队列与超时控制。

## 项目能力概览
- SystemJS 构建管线：esbuild + SWC，Tailwind 可选
- 页面 schema → TSX 代码（component-generator）
- manifest → 项目虚拟文件（project-generator）
- 虚拟文件 → 构建产物（code-export）
- 统一编排（forge-core）+ HTTP/CLI 服务（server）

## 核心流程
1. component-generator：page schema → TSX 代码片段
2. project-generator：manifest + page renderer → VirtualFile[]
3. code-export：VirtualFile[] → SystemJS JS (+ 可选 CSS)
4. forge-core：统一编排 API，server 提供 HTTP/CLI 入口

## Monorepo 结构
- `apps/server`：HTTP/CLI 入口（Fastify 服务、缓存/队列等）
- `packages/vendor`：构建阶段需要的第三方依赖集合（独立安装）
- `packages/forge-core`：编排层（项目生成 + 构建）
- `packages/code-export`：SystemJS 构建管线包
- `packages/project-generator`：根据 manifest + 组件代码生成项目骨架
- `packages/component-generator`：从组件树生成 TSX 代码（仍在演进）
- `spec/`：设计与接口草稿

## 快速开始
```bash
pnpm install

# vendor 依赖（server 构建时需要）
pnpm --filter @frontend-forge/vendor deploy --prod apps/server/vendor

# 本地开发（热重载）
pnpm --filter @frontend-forge/server dev
```

服务默认监听 `0.0.0.0:3000`，健康检查 `GET /healthz`。

## 常用命令
- `pnpm --filter @frontend-forge/server dev`：开发模式（tsx watch）
- `pnpm --filter @frontend-forge/server build`：编译 server 到 `apps/server/dist`
- `pnpm --filter @frontend-forge/server start`：build 后运行 server
- `pnpm -r run build`：构建所有有 build 脚本的包
- `apps/server/test.sh`：集成测试（需本地 server + curl/jq/rg/tar）

## 服务端 API（概览）
- `POST /build`：上传源码并返回 SystemJS 产物
- `POST /page/code`：page schema → TS/TSX 代码
- `POST /project/files`：manifest → VirtualFile[]
- `POST /project/files.tar.gz`：manifest → tar.gz 源码包
- `POST /api/project/build`：manifest → SystemJS 产物数组
- `POST /api/project/build.tar.gz`：manifest → tar.gz 产物包

详细请求/响应示例见 `apps/server/README.md`。

## 配置与缓存
环境变量（`apps/server/src/config.ts`）：
- `PORT`：HTTP 端口，默认 3000
- `CACHE_DIR`：磁盘缓存目录，默认 `.cache`
- `CACHE_MAX_ITEMS`：内存缓存大小，默认 200
- `CONCURRENCY`：并发构建数，默认 `cpu/2`
- `BUILD_TIMEOUT_MS`：构建超时，默认 30000
- `MAX_BODY_BYTES`：请求体大小，默认 1 MiB
- `CHILD_MAX_OLD_SPACE_MB`：子进程 Node 最大内存，默认 512

临时构建输出写入 `.tmp/`，不应提交到仓库。

## 示例与本地验证
- `apps/server/examples/build.request.json`：`/build` 请求示例
- `apps/server/examples/page.schema.json`：`/page/code` 请求示例
- `apps/server/examples/manifest.test.json`：`/project/*` 请求示例
- `apps/server/test.sh`：端到端接口验证脚本

## 子包说明
- `packages/forge-core`：稳定入口，封装 generate/build/emit API
- `packages/code-export`：可复用的 SystemJS 构建管线
- `packages/project-generator`：模板渲染与 manifest 校验
- `packages/component-generator`：schema → TSX 生成器（完善中）

每个包的详细设计与使用方式请参阅其 `README.md`。

## 开发约定
- TypeScript + ESM（`type: module`），使用 `import`/`export`
- 2 空格缩进 + 分号，保持与相邻代码一致
- server 相对 ESM 导入需保留 `.js` 扩展名
