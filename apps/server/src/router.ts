import type { FastifyInstance } from "fastify";
import type { ForgeCore } from "@frontend-forge/forge-core";
import type {
  BuildRequestBody,
  PageSchemaRequestBody,
  ProjectJsBundleRequestBody,
  ProjectManifestRequestBody,
} from "./types.js";
import { createController } from "./controller/index.js";
import type { K8sConfig } from "./runtimeConfig.js";

export type RouterOptions = {
  forge: ForgeCore;
  k8s?: K8sConfig;
};

export default async function router(
  app: FastifyInstance,
  opts: RouterOptions,
) {
  const controller = createController(opts);
  const API_PREFIX = "/api";
  const withApiPrefix = (path: string) => `${API_PREFIX}${path}`;

  app.get("/healthz", controller.healthz);

  app.post<{ Body: BuildRequestBody }>(
    withApiPrefix("/build"),
    async (req, reply) => {
      return controller.build(req.body, reply);
    },
  );

  app.post<{ Body: PageSchemaRequestBody }>(
    withApiPrefix("/page/code"),
    async (req, reply) => {
      return controller.pageCode(req.body, reply);
    },
  );

  app.post<{ Body: ProjectManifestRequestBody }>(
    withApiPrefix("/project/files"),
    async (req, reply) => {
      return controller.projectFiles(req.body, reply);
    },
  );

  app.post<{ Body: ProjectManifestRequestBody }>(
    withApiPrefix("/project/files.tar.gz"),
    async (req, reply) => {
      return controller.projectFilesTarGz(req.body, reply);
    },
  );

  app.post<{ Body: ProjectManifestRequestBody }>(
    withApiPrefix("/project/build"),
    async (req, reply) => {
      return controller.projectBuild(req.body, reply);
    },
  );

  app.post<{ Body: ProjectManifestRequestBody }>(
    withApiPrefix("/project/build.tar.gz"),
    async (req, reply) => {
      return controller.projectBuildTarGz(req.body, reply);
    },
  );

  app.post<{ Body: ProjectJsBundleRequestBody }>(
    withApiPrefix("/k8s/jsbundles"),
    async (req, reply) => {
      return controller.projectJsBundle(req, reply);
    },
  );
}
