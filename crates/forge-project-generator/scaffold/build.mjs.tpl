import { existsSync } from 'node:fs';
import { cp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { build as rolldownBuild } from 'rolldown';
import { minifySync as rolldownMinifySync } from 'rolldown/experimental';

const buildFormat = '__BUILD_FORMAT__';
const externalPackages = __EXTERNAL_PACKAGES__;

if (!['esm', 'systemjs'].includes(buildFormat)) {
  throw new Error(`unsupported build format: ${buildFormat}`);
}

const devModeValues = new Set(['1', 'true', 'yes', 'on', 'dev', 'development']);
const isForgeDevMode = () =>
  devModeValues.has((process.env.FORGE_DEV_MODE ?? '').toLowerCase()) ||
  process.env.NODE_ENV === 'development';

const rootDir = dirname(fileURLToPath(import.meta.url));
const tempDir = join(rootDir, '.forge-rolldown');
const distDir = join(rootDir, 'dist');
const tempEntry = join(tempDir, 'index.js');
const tempStyle = join(tempDir, 'index.css');
const forgeComponentsSourceEntry = fileURLToPath(
  new URL('./node_modules/@frontend-forge/forge-components/src/index.ts', import.meta.url),
);

const hasForgeComponentsSource = () => isForgeDevMode() && existsSync(forgeComponentsSourceEntry);

const useSyncExternalStoreShimSource = __USE_SYNC_EXTERNAL_STORE_SHIM_SOURCE__;
const useSyncExternalStoreWithSelectorSource = __USE_SYNC_EXTERNAL_STORE_WITH_SELECTOR_SOURCE__;

const reactCjsShimModules = new Map([
  ['use-sync-external-store', useSyncExternalStoreShimSource],
  ['use-sync-external-store/index.js', useSyncExternalStoreShimSource],
  ['use-sync-external-store/shim', useSyncExternalStoreShimSource],
  ['use-sync-external-store/shim/index.js', useSyncExternalStoreShimSource],
  ['use-sync-external-store/with-selector', useSyncExternalStoreWithSelectorSource],
  ['use-sync-external-store/with-selector.js', useSyncExternalStoreWithSelectorSource],
  ['use-sync-external-store/shim/with-selector', useSyncExternalStoreWithSelectorSource],
  ['use-sync-external-store/shim/with-selector.js', useSyncExternalStoreWithSelectorSource],
]);
const forgeBuildExternalPackages = ['esprima'];

function resolveForgeComponentsSource() {
  return {
    name: 'forge-components-source',
    resolveId(source) {
      if (!hasForgeComponentsSource()) {
        return;
      }
      if (source === '@frontend-forge/forge-components') {
        return forgeComponentsSourceEntry;
      }
    },
  };
}

function resolveReactCjsShims() {
  const namespace = '\0forge-react-cjs-shims:';
  return {
    name: 'react-cjs-shims-to-esm',
    resolveId(source) {
      if (reactCjsShimModules.has(source)) {
        return `${namespace}${source}`;
      }
    },
    load(id) {
      if (!id.startsWith(namespace)) {
        return;
      }
      return {
        code: reactCjsShimModules.get(id.slice(namespace.length)),
        moduleType: 'js',
      };
    },
  };
}

function isExternalPackage(id) {
  return [...externalPackages, ...forgeBuildExternalPackages].some((name) => id === name || id.startsWith(`${name}/`));
}

function rolldownAssetModuleTypes() {
  return {
    '.avif': 'asset',
    '.gif': 'asset',
    '.jpg': 'asset',
    '.jpeg': 'asset',
    '.png': 'asset',
    '.svg': 'asset',
    '.webp': 'asset',
    '.woff': 'asset',
    '.woff2': 'asset',
    '.ttf': 'asset',
    '.eot': 'asset',
  };
}

function minifyWithRolldown(code) {
  return rolldownMinifySync('index.js', code, {
    sourcemap: false,
  }).code;
}

async function timeStage(stage, task, logInfo = console.log, logError = console.error) {
  const started = performance.now();
  try {
    const value = await task();
    logInfo(`forge build stage completed stage=${stage} elapsed_ms=${Math.round(performance.now() - started)}`);
    return value;
  } catch (error) {
    logError(`forge build stage failed stage=${stage} elapsed_ms=${Math.round(performance.now() - started)}`);
    throw error;
  }
}

async function copyBuildOutputs() {
  await mkdir(distDir, { recursive: true });
  if (existsSync(tempStyle)) {
    await cp(tempStyle, join(distDir, 'style.css'));
  }
  const tempAssets = join(tempDir, 'assets');
  if (existsSync(tempAssets)) {
    await cp(tempAssets, join(distDir, 'assets'), { recursive: true });
  }
}

async function bundleEsm(stageTimer) {
  await stageTimer('rolldown_bundle', () =>
    rolldownBuild({
      cwd: rootDir,
      input: 'src/index.ts',
      external: isExternalPackage,
      platform: 'browser',
      treeshake: true,
      moduleTypes: rolldownAssetModuleTypes(),
      transform: {
        define: {
          'process.env.NODE_ENV': JSON.stringify('production'),
        },
        jsx: 'react',
      },
      output: {
        dir: tempDir,
        entryFileNames: 'index.js',
        assetFileNames: 'assets/[name]-[hash][extname]',
        chunkFileNames: 'assets/[name]-[hash].js',
        format: 'esm',
        minify: true,
        sourcemap: false,
      },
      plugins: [
        resolveForgeComponentsSource(),
        resolveReactCjsShims(),
      ],
    }),
  );
}

async function toSystemjs(bundled, stageTimer) {
  const { transform: swcTransform } = await import('@swc/core');
  const output = await stageTimer('swc_systemjs', () =>
    swcTransform(bundled, {
      filename: 'index.js',
      jsc: {
        parser: {
          syntax: 'ecmascript',
        },
        target: 'es2022',
      },
      module: {
        type: 'systemjs',
      },
      minify: false,
      sourceMaps: false,
    }),
  );
  return output.code;
}

async function minifyJs(code, stageTimer) {
  return stageTimer('rolldown_minify', () => minifyWithRolldown(code));
}

export async function runBuild(options = {}) {
  const logInfo = options.onLog ?? console.log;
  const logError = options.onError ?? console.error;
  const stageTimer = (stage, task) => timeStage(stage, task, logInfo, logError);

  await rm(tempDir, { recursive: true, force: true });
  await rm(distDir, { recursive: true, force: true });
  await bundleEsm(stageTimer);

  const bundled = await readFile(tempEntry, 'utf8');
  const outputCode = buildFormat === 'systemjs'
    ? await minifyJs(await toSystemjs(bundled, stageTimer), stageTimer)
    : bundled;

  await mkdir(distDir, { recursive: true });
  await writeFile(join(distDir, 'index.js'), outputCode);
  await stageTimer('copy_assets', copyBuildOutputs);
}

let isMain = false;
try {
  if (process.argv[1]) {
    isMain = import.meta.url === pathToFileURL(process.argv[1]).href;
  }
} catch {}

if (isMain) {
  runBuild().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
