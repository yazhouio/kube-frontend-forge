import terser from '@rollup/plugin-terser';
import { createBaseConfig } from './rollup.base.config.mjs';

export default createBaseConfig({
  postPlugins: [
    terser({
      format: {
        comments: false,
      },
    }),
  ],
});
