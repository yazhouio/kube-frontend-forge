const devModeValues = new Set(['1', 'true', 'yes', 'on', 'dev', 'development']);

const isForgeDevMode = () =>
  devModeValues.has((process.env.FORGE_DEV_MODE ?? '').toLowerCase()) ||
  process.env.NODE_ENV === 'development';

const configModule = isForgeDevMode()
  ? await import('./rollup.dev.config.mjs')
  : await import('./rollup.prod.config.mjs');

export default configModule.default;
