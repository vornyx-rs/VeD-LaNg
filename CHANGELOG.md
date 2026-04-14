# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-04-13

### Added

- **Web WASM compiler** — `vedc build --target web` emits Rust source compiled to `app.wasm`
  via `wasm32-unknown-unknown`. Canvas 2D draw calls are wired as WASM imports. The JS
  bootstrap is a ~40-line shell; no JS framework is involved.
- **Tap dispatch** — `tap` buttons inside `box` nodes produce `push_tap()` hit-test rects
  each frame. `dispatch(id)` routes events back into the WASM state machine.
- **Layout engine** — column (`flow: down`) and row (`flow: across`) layout with gap, center,
  and padding support emitted directly into the WASM render functions.
- **Spring physics** — CPU spring solver (`stiffness`, `damping`, `mass`) embedded in generated
  WASM. Feature-gated WebGPU backend stub also present (`--features webgpu`).
- **Reactive `remember` signals** — computed dependencies resolved at parse time; `AppState`
  struct generated from `remember` declarations per screen.
- **`fetch ... when`** — declarative data fetching with re-fetch on dependency change, caching,
  loading and error states.
- **`each` UI node** — list rendering in canvas UI (`each item in list`).
- **`when expr` UI conditional** — conditional node rendering.
- **`think` UI body** — `think` functions can contain `box`/`words` nodes; emits
  `render_piece_*()` alongside the logic function.
- **Auto WASM build** — `vedc build --target web` invokes `cargo build --target wasm32-unknown-unknown`
  automatically and copies `app.wasm` to the output directory.
- **VS Code extension** — full extension in `editors/vscode/` with syntax highlighting,
  snippets, command palette integration, and configurable compiler path.
- **`audit.toml`** — three accepted advisories documented with justification; `cargo audit`
  exits 0 with `--ignore` flags.

### Changed

- Upgraded `sqlx` from 0.7 to 0.8 (`runtime-tokio` + `tls-rustls` feature split).
- Removed `mysql` from sqlx features (RUSTSEC-2023-0071 in `rsa 0.9.x`, no upstream fix;
  MySQL planned for v1.1).
- Removed `vedc update` subcommand (was a stub printing "not yet implemented").
- `runtime::web::start` verbose output gated behind `--verbose` flag; unconditional
  debug dumps removed.
- All clippy warnings resolved; `cargo clippy -- -D warnings` is clean.
- `cargo fmt` is clean.

### Fixed

- `count - 1` parsed as `count(-1)` due to `Token::Minus` triggering no-paren call path;
  excluded `Minus` from `is_expr_start()` in `parse_primary`.
- `parse_tap`: hover/enter/leave/animate/padding handling added; silent token stream
  corruption for second-and-subsequent tap+hover blocks in same box fixed.
- `parse_animate_block`: accepts indented-block style alongside brace style.
- `parse_each_node`: implemented `each var in expr` UI node.
- `parse_show_when`: implemented `when expr` UI conditional node.
- `parse_remember`: consumes optional `when:` / `cache:` reactive options block.
- `expect_ident`: accepts property keywords as contextual identifiers.
- Duplicate match arms in `Token::Display` impl removed (unreachable pattern errors).
- `EOF` → `Eof`, `LWW` → `Lww` to satisfy clippy acronym lint.

### Security

- `cargo audit` clean (three advisories acknowledged in `audit.toml` with justification;
  none affect VED's compiled code paths directly).

### Known Limitations (v1)

- `each` in canvas UI: loop variable not passed into child expression context.
- `parse_time` returns 0 (chrono integration pending).
- CRDT live collaboration: parsed and emitted in JS; no Rust server-side runtime.
- LSP (`vedc lsp`): scaffold only — protocol loop not implemented.
- `vedc fmt` / `vedc test`: parse-only; formatter and test runner not implemented.
- Ghost AI mode: intent capture infrastructure present; AI engine not connected.
- MySQL: removed from v1; planned for v1.1 pending upstream RSA fix.

## [0.1.0] - 2024-01-15

### Added (initial release)

- Initial release of VED language compiler.
- Lexer with Python-style indentation tracking using logos.
- Recursive descent parser with full AST support.
- Type checker with scoped TypeEnv and builtin resolution.
- Tree-walk interpreter for development mode.
- Code generation backends for web (HTML/JS), native (rustc), and server (Axum).
- `maybe[T]` for null safety; `ok[T, E]` for error handling.
- Pipe operator `|>` for function composition.
- String interpolation `"Hello {name}"`.
- UI components: `screen`, `piece`, `box`, `words`, `tap`, `field`, `image`.
- Server definitions with HTTP routes (GET/POST/PUT/PATCH/DEL).
- Database integration primitives (SQLite, Postgres, MySQL).
- WebSocket support with `live` blocks.
- Authentication with JWT (HS256).
- Standard library with 50+ builtin functions.
- CLI with `run`, `build`, `check`, `fmt`, `test`, `new` commands.
- MIT and Apache-2.0 dual licensing.

[1.0.0]: https://github.com/ved-lang/ved/compare/v0.1.0...v1.0.0
[0.1.0]: https://github.com/ved-lang/ved/releases/tag/v0.1.0
