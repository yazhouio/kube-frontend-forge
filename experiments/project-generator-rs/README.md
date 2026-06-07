# project-generator-rs

Experimental Rust rewrite of `packages/project-generator`.

The crate mirrors the current TypeScript generator responsibilities:

- validate and normalize an extension manifest
- copy scaffold files
- render `package.json`, `extensionConfig`, routes, locales, and page files
- merge scaffold default locales with manifest locales
- return generated `VirtualFile` values and warnings

The page body is supplied by a `PageRenderer` callback in library usage. The CLI uses a minimal placeholder page renderer, so it is mostly for testing the project-file pipeline until it is wired to `component-generator-rs`.

## Usage

```bash
cargo run -- ../../apps/server/examples/manifest.sample.json
cargo run -- ../../apps/server/examples/manifest.sample.json --out ../../.tmp/project-generator-rs.json
cargo test
```
