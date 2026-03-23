import type { ControllerDeps } from './deps.js';
import { createProjectHandlers } from './projectHandlers.js';

export type { ControllerDeps };

export function createController(deps: ControllerDeps) {
  return {
    healthz: async () => ({ ok: true }),
    ...createProjectHandlers(deps),
  };
}
