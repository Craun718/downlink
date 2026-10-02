# TODO

## JavaScript bindings

 - [ ] Add an napi-rs binding crate for Node.js, Bun, Deno, and Electron.
 - [ ] Keep the native interface small: opaque engine handle, JSON input/output, typed errors, and Promise-returning send.
 - [ ] Provide a TypeScript facade with runtime-agnostic object APIs.
 - [ ] Publish platform packages through `optionalDependencies`.
 - [ ] Use CI to establish first-party support for all four runtimes:
   - [ ] Node.js
   - [ ] Bun
   - [ ] Deno with local `node_modules` and `--allow-ffi`
   - [ ] Electron main-process smoke test
 - [ ] Prefer rustls or statically linked TLS for portable prebuilt binaries.
 - [ ] Document Electron packaging requirements, including `asarUnpack` for `.node` files.
 - [ ] Document Deno permission implications, especially `--allow-ffi`.

## Rust core

 - [ ] Add a high-level engine interface that owns the Tokio runtime and HTTP client.
 - [ ] Keep Rust-specific trait objects and async types out of language bindings.
