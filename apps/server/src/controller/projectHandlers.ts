import type { FastifyReply, FastifyRequest } from 'fastify';
import type {
  BuildRequestBody,
  PageSchemaRequestBody,
  ProjectJsBundleRequestBody,
  ProjectManifestRequestBody,
} from '../types.js';
import type { ControllerDeps } from './deps.js';
import { handleKnownError } from './errors.js';
import { createJsBundleFromManifest } from './jsbundle.js';
import { requireAuthToken, requireK8sConfig } from './k8s.js';
import {
  requireExporter,
  requireJsBundleParams,
  requireManifest,
  requirePageSchema,
} from './validation.js';

export function createProjectHandlers({ forge, k8s }: ControllerDeps) {
  return {
    build: async (body: BuildRequestBody, reply: FastifyReply) => {
      try {
        const exporter = requireExporter(forge);
        const fileCount = Array.isArray(body?.files) ? body.files.length : 0;
        reply.log.info(
          {
            entry: body?.entry,
            fileCount,
            externalsCount: Array.isArray(body?.externals) ? body.externals.length : 0,
            tailwindEnabled: Boolean(body?.tailwind?.enabled),
          },
          'Build requested'
        );
        const result = await exporter.build(body);
        reply.log.info(
          { cacheHit: result.cacheHit ?? false, key: result.key, meta: result.meta },
          'Build completed'
        );
        return { ok: true, ...result, cacheHit: result.cacheHit ?? false };
      } catch (err) {
        return handleKnownError(err, reply);
      }
    },

    pageCode: async (body: PageSchemaRequestBody, reply: FastifyReply) => {
      try {
        reply.log.info('Page code generation requested');
        const schema = requirePageSchema(body);
        const code = forge.generatePageCode(schema);
        reply.log.info({ codeSize: code.length }, 'Page code generation completed');
        return { ok: true, code };
      } catch (err) {
        return handleKnownError(err, reply);
      }
    },

    projectFiles: async (body: ProjectManifestRequestBody, reply: FastifyReply) => {
      try {
        const manifest = requireManifest(body);
        reply.log.info(
          { name: manifest.name, pages: manifest.pages?.length ?? 0, routes: manifest.routes?.length ?? 0 },
          'Project files generation requested'
        );
        const files = await forge.generateProjectFiles(manifest);
        reply.log.info({ fileCount: files.length }, 'Project files generation completed');
        return { ok: true, files };
      } catch (err) {
        return handleKnownError(err, reply);
      }
    },

    projectFilesTarGz: async (body: ProjectManifestRequestBody, reply: FastifyReply) => {
      try {
        const manifest = requireManifest(body);
        reply.log.info(
          { name: manifest.name, pages: manifest.pages?.length ?? 0, routes: manifest.routes?.length ?? 0 },
          'Project tar.gz generation requested'
        );
        const files = await forge.generateProjectFiles(manifest);
        const archive = forge.emitToTarGz(files);
        reply.header('Content-Type', 'application/gzip');
        reply.header('Content-Disposition', 'attachment; filename="project.tar.gz"');
        reply.log.info({ fileCount: files.length, size: archive.byteLength }, 'Project tar.gz generation completed');
        return reply.send(archive);
      } catch (err) {
        return handleKnownError(err, reply);
      }
    },

    projectBuild: async (body: ProjectManifestRequestBody, reply: FastifyReply) => {
      try {
        const manifest = requireManifest(body);
        reply.log.info(
          { name: manifest.name, pages: manifest.pages?.length ?? 0, routes: manifest.routes?.length ?? 0 },
          'Project build requested'
        );
        const files = await forge.buildProject(manifest, { build: true });
        reply.log.info({ fileCount: files.length }, 'Project build completed');
        return { ok: true, files };
      } catch (err) {
        return handleKnownError(err, reply);
      }
    },

    projectBuildTarGz: async (body: ProjectManifestRequestBody, reply: FastifyReply) => {
      try {
        const manifest = requireManifest(body);
        reply.log.info(
          { name: manifest.name, pages: manifest.pages?.length ?? 0, routes: manifest.routes?.length ?? 0 },
          'Project build tar.gz requested'
        );
        const files = await forge.buildProject(manifest, { build: true });
        const archive = forge.emitToTarGz(files);
        reply.header('Content-Type', 'application/gzip');
        reply.header('Content-Disposition', 'attachment; filename="build.tar.gz"');
        reply.log.info({ fileCount: files.length, size: archive.byteLength }, 'Project build tar.gz completed');
        return reply.send(archive);
      } catch (err) {
        return handleKnownError(err, reply);
      }
    },

    projectJsBundle: async (
      req: FastifyRequest<{ Body: ProjectJsBundleRequestBody }>,
      reply: FastifyReply
    ) => {
      try {
        const k8sConfig = requireK8sConfig(k8s);
        const token = requireAuthToken(req, k8sConfig);
        const { name, extensionName, namespace, cluster } = requireJsBundleParams(req.body);
        const manifest = requireManifest(req.body);
        return createJsBundleFromManifest({
          forge,
          k8s: k8sConfig,
          token: token,
          manifest,
          name,
          extensionName,
          namespace,
          cluster,
          reply,
        });
      } catch (err) {
        return handleKnownError(err, reply);
      }
    },
  };
}
