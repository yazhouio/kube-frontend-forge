{
  "name": "__NAME__",
  "version": "__VERSION__",
  "private": true,
  "type": "module",
  "main": "dist/index.js",
  "scripts": {
    "build": "rollup -c"
  },
  "dependencies": {
__DEPENDENCIES__
  },
  "devDependencies": {
    "@rollup/plugin-commonjs": "^29.0.3",
    "@rollup/plugin-json": "^6.1.0",
    "@rollup/plugin-node-resolve": "^16.0.3",
    "esbuild": "^0.27.2",
    "postcss": "^8.5.6",
    "rollup": "^4.61.1",
    "rollup-plugin-postcss": "^4.0.2"
  }
}
