import { existsSync } from 'node:fs';
import { cp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { build, transform as esbuildTransform } from 'esbuild';

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
const tempDir = join(rootDir, '.forge-esbuild');
const distDir = join(rootDir, 'dist');
const tempEntry = join(tempDir, 'index.js');
const tempStyle = join(tempDir, 'index.css');
const forgeComponentsSourceEntry = fileURLToPath(
  new URL('./node_modules/@frontend-forge/forge-components/src/index.ts', import.meta.url),
);

const external = externalPackages.flatMap((name) => [name, `${name}/*`]);
const hasForgeComponentsSource = () => isForgeDevMode() && existsSync(forgeComponentsSourceEntry);

function resolveForgeComponentsSource() {
  return {
    name: 'forge-components-source',
    setup(build) {
      if (!hasForgeComponentsSource()) {
        return;
      }
      build.onResolve({ filter: /^@frontend-forge\/forge-components$/ }, () => ({
        path: forgeComponentsSourceEntry,
      }));
    },
  };
}

async function timeStage(stage, task) {
  const started = performance.now();
  try {
    const value = await task();
    console.log(`forge build stage completed stage=${stage} elapsed_ms=${Math.round(performance.now() - started)}`);
    return value;
  } catch (error) {
    console.error(`forge build stage failed stage=${stage} elapsed_ms=${Math.round(performance.now() - started)}`);
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

async function bundleEsm() {
  await timeStage('esbuild_bundle', () =>
    build({
      entryPoints: ['src/index.ts'],
      outfile: tempEntry,
      bundle: true,
      format: 'esm',
      platform: 'browser',
      target: 'es2022',
      jsx: 'transform',
      treeShaking: true,
      minify: buildFormat === 'esm',
      charset: 'utf8',
      legalComments: 'none',
      sourcemap: false,
      assetNames: 'assets/[name]-[hash]',
      external,
      define: {
        'process.env.NODE_ENV': JSON.stringify('production'),
      },
      loader: {
        '.avif': 'file',
        '.gif': 'file',
        '.jpg': 'file',
        '.jpeg': 'file',
        '.png': 'file',
        '.svg': 'file',
        '.webp': 'file',
        '.woff': 'file',
        '.woff2': 'file',
        '.ttf': 'file',
        '.eot': 'file',
      },
      plugins: [resolveForgeComponentsSource()],
    }),
  );
}

async function toSystemjs(bundled) {
  const { transform: swcTransform } = await import('@swc/core');
  const output = await timeStage('swc_systemjs', () =>
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

async function minifyJs(code) {
  const output = await timeStage('esbuild_minify', () =>
    esbuildTransform(code, {
      loader: 'js',
      target: 'es2022',
      minify: true,
      charset: 'utf8',
      legalComments: 'none',
      sourcemap: false,
    }),
  );
  return output.code;
}

async function runBuild() {
  await rm(tempDir, { recursive: true, force: true });
  await rm(distDir, { recursive: true, force: true });
  await bundleEsm();

  const bundled = await readFile(tempEntry, 'utf8');
  const outputCode = buildFormat === 'systemjs'
    ? await minifyJs(await toSystemjs(bundled))
    : bundled;

  await mkdir(distDir, { recursive: true });
  await writeFile(join(distDir, 'index.js'), outputCode);
  await timeStage('copy_assets', copyBuildOutputs);
}

runBuild().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
