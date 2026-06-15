{
  "name": "__NAME__",
  "version": "__VERSION__",
  "private": true,
  "type": "module",
  "main": "dist/index.js",
  "scripts": {
    "build": "node build.mjs"
  },
  "dependencies": {
__DEPENDENCIES__
  },
  "devDependencies": {
    "@swc/core": "^1.15.41",
    "esbuild": "^0.27.2",
    "postcss": "^8.5.6"
  }
}
