import type { FastifyInstance } from "fastify";
import type { ForgeCore } from "@frontend-forge/forge-core";
import type {
  BuildRequestBody,
  PageSchemaRequestBody,
  ProjectJsBundleRequestBody,
  ProjectManifestRequestBody,
  SceneRequestBody,
  SceneJsBundleRequestBody,
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

  app.get("/healthz", controller.healthz);

  app.post<{ Body: BuildRequestBody }>("/build", async (req, reply) => {
    return controller.build(req.body, reply);
  });

  app.post<{ Body: PageSchemaRequestBody }>(
    "/page/code",
    async (req, reply) => {
      return controller.pageCode(req.body, reply);
    },
  );

  app.post<{ Body: ProjectManifestRequestBody }>(
    "/project/files",
    async (req, reply) => {
      return controller.projectFiles(req.body, reply);
    },
  );

  app.post<{ Body: ProjectManifestRequestBody }>(
    "/project/files.tar.gz",
    async (req, reply) => {
      return controller.projectFilesTarGz(req.body, reply);
    },
  );

  app.post<{ Body: ProjectManifestRequestBody }>(
    "/api/project/build",
    async (req, reply) => {
      return controller.projectBuild(req.body, reply);
    },
  );

  app.post<{ Body: ProjectManifestRequestBody }>(
    "/api/project/build.tar.gz",
    async (req, reply) => {
      return controller.projectBuildTarGz(req.body, reply);
    },
  );

  app.post<{ Body: SceneRequestBody }>(
    "/scene/project/files",
    async (req, reply) => {
      return controller.sceneProjectFiles(req.body, reply);
    },
  );

  app.post<{ Body: SceneRequestBody }>(
    "/scene/project/files.tar.gz",
    async (req, reply) => {
      return controller.sceneProjectFilesTarGz(req.body, reply);
    },
  );

  app.post<{ Body: SceneRequestBody }>(
    "/scene/project/build",
    async (req, reply) => {
      return controller.sceneProjectBuild(req.body, reply);
    },
  );

  app.post<{ Body: SceneRequestBody }>(
    "/scene/project/build.tar.gz",
    async (req, reply) => {
      return controller.sceneProjectBuildTarGz(req.body, reply);
    },
  );

  app.post<{ Body: ProjectJsBundleRequestBody }>(
    "/k8s/jsbundles",
    async (req, reply) => {
      return controller.projectJsBundle(req, reply);
    },
  );

  app.post<{ Body: SceneJsBundleRequestBody }>(
    "/k8s/jsbundles/scene",
    async (req, reply) => {
      return controller.sceneJsBundle(req, reply);
    },
  );

  app.get<{
    Querystring: {
      enabled?: string;
      type?: string;
      name?: string;
      sortBy?: string;
      ascending?: string;
    };
  }>(
    "/kapis/frontend-forge.io/v1alpha1/frontendintegrations",
    async (req, reply) => {
      return controller.frontendIntegrationList(req, reply);
    },
  );

  app.post<{ Body: unknown }>(
    "/kapis/frontend-forge.io/v1alpha1/frontendintegrations",
    async (req, reply) => {
      return controller.frontendIntegrationCreate(req, reply);
    },
  );

  app.get<{ Params: { name: string } }>(
    "/kapis/frontend-forge.io/v1alpha1/frontendintegrations/:name",
    async (req, reply) => {
      return controller.frontendIntegrationGet(req, reply);
    },
  );

  app.put<{ Params: { name: string }; Body: unknown }>(
    "/kapis/frontend-forge.io/v1alpha1/frontendintegrations/:name",
    async (req, reply) => {
      return controller.frontendIntegrationUpdate(req, reply);
    },
  );

  app.delete<{ Params: { name: string } }>(
    "/kapis/frontend-forge.io/v1alpha1/frontendintegrations/:name",
    async (req, reply) => {
      return controller.frontendIntegrationDelete(req, reply);
    },
  );

  app.put<{ Params: { name: string }; Body: { enabled: boolean } }>(
    "/kapis/frontend-forge.io/v1alpha1/frontendintegrations/:name/enabled",
    async (req, reply) => {
      return controller.frontendIntegrationUpdateEnabled(req, reply);
    },
  );
}
