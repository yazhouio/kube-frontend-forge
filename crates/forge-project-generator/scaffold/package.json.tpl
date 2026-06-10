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
    "@rollup/plugin-terser": "^0.4.4",
    "@rollup/plugin-typescript": "^12.3.0",
    "postcss": "^8.5.6",
    "rollup": "^4.61.1",
    "rollup-plugin-postcss": "^4.0.2",
    "tslib": "^2.8.1",
    "typescript": "^5.7.3"
  }
}
