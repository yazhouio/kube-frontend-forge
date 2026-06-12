import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { createBaseConfig } from './rollup.base.config.mjs';

const forgeComponentsSourceEntry = fileURLToPath(
  new URL('./node_modules/@frontend-forge/forge-components/src/index.ts', import.meta.url),
);

function resolveForgeComponentsSource() {
  const useSource = existsSync(forgeComponentsSourceEntry);

  return {
    name: 'forge-components-source',
    resolveId(id) {
      if (useSource && id === '@frontend-forge/forge-components') {
        return forgeComponentsSourceEntry;
      }
      return null;
    },
  };
}

export default createBaseConfig({
  prePlugins: [resolveForgeComponentsSource()],
});
