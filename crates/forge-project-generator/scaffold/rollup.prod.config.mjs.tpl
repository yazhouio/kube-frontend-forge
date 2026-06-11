import { createBaseConfig, esbuildMinify } from './rollup.base.config.mjs';

export default createBaseConfig({
  postPlugins: [esbuildMinify()],
});
