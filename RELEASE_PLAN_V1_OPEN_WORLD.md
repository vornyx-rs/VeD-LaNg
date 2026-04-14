# VED v1.0 Open-World Release Plan

Date: 2026-04-12

This plan is grounded in the **current repository state** and validated local checks.

## 1) Release Goal

Ship VED `v1.0.0` for open-world testing with clear go/no-go criteria, stable core workflows, and enough operational guardrails to gather actionable community feedback.

## 2) Verified Current State (from repo)

### Versioning and metadata

- `Cargo.toml` version: `1.0.0`
- `CHANGELOG.md` includes `## [1.0.0] - 2026-04-12`
- Release profile optimization already configured (`lto`, `codegen-units=1`, `strip`, `panic=abort`)

### Feature surface visible in code/tests

- Reactive state + computed semantics in parser/typeck/interpreter tests
- `fetch ... when` semantics covered in parser/interpreter/typeck tests
- Layout primitives (`center`, `layer`, `backdrop`) in parser/compiler tests
- Animation runtime present in `src/runtime/animations.rs` with CPU spring + optional `webgpu` backend
- 43 unit tests currently passing

### CLI commands/targets (actual)

From `src/main.rs`:

- Commands: `run`, `build`, `check`, `fmt`, `test`, `ast`, `lex`, `lsp`, `new`, `update`
- Build targets: `auto`, `web`, `bin`, `server`, `all`

> Note: This differs from earlier draft wording like `--target fullstack`.

### Examples currently present

Flat files under `examples/` (e.g. `hello.ved`, `counter.ved`, `todo.ved`, `websocket.ved`, etc.).

## 3) Quality Gates (validated today)

| Gate | Result | Evidence |
| --- | --- | --- |
| Build (`cargo build`) | PASS | Completed successfully |
| Tests (`cargo test`) | PASS | `43 passed; 0 failed` |
| Lint (`cargo clippy --all-targets --all-features`) | FAIL | Warnings present (~133), including `clippy::upper_case_acronyms`, `redundant_closure`, dead code, unused vars |

Go-live recommendation: **allow warnings for 1.0 open-world**, but track top warnings as a post-release cleanup milestone.

## 4) v1.0 Release Contract

A release candidate is acceptable when all of the following are true:

1. `cargo build` and `cargo test` are green on CI.
2. Core examples (`hello`, `counter`, `todo`, `stream`, `websocket`) run or build successfully.
3. Release artifacts are produced for Linux/macOS/Windows.
4. Basic security review completed for auth/database entry points.
5. Documentation and quickstart reflect **actual** CLI targets (`web|bin|server|all`).

## 5) Open-World Testing Scope

### In scope

- Compiler correctness for language constructs already covered by tests
- Web + native + server build paths
- Parser/type-check/runtime behavior for reactive + fetch + animation syntax
- DX feedback: error quality, examples discoverability, onboarding friction

### Out of scope (post-1.0 stabilization)

- Zero-warning lint baseline
- Full benchmark publication with strict regression budgets
- Large-scale plugin/LSP polish

## 6) Launch Week Checklist

### T-5 to T-3 days (stabilize)

- [ ] Freeze language syntax for v1.0 patch line
- [ ] Verify examples compile/run in a fresh environment
- [ ] Add `install.sh` only after release artifact naming is finalized
- [ ] Confirm release note claims match tested behavior

### T-2 to T-1 days (release candidate)

- [ ] Cut `v1.0.0-rc1` tag
- [ ] Build artifacts for:
  - [ ] Linux x86_64
  - [ ] Linux aarch64
  - [ ] macOS x86_64
  - [ ] macOS aarch64
  - [ ] Windows x86_64
- [ ] Smoke test each artifact (`vedc --version`, run one example)

### Release day

- [ ] Tag `v1.0.0`
- [ ] Publish release notes + checksums
- [ ] Publish announcement post
- [ ] Start open-world feedback intake issue template

### Post-release (first 14 days)

- [ ] Triage incoming bug reports daily
- [ ] Track top onboarding blockers
- [ ] Ship `v1.0.1` with high-impact fixes only

## 7) Risk Register

1. **Warning debt hides regressions**
   - Mitigation: add targeted deny-list in CI for severe lints over time.
2. **Examples drift from parser behavior**
   - Mitigation: add example compile test matrix.
3. **Mismatch between docs and CLI**
   - Mitigation: doc checklist item requires command/target verification against `src/main.rs`.
4. **Optional WebGPU variability across machines**
   - Mitigation: keep CPU fallback default and clearly document feature-flag behavior.

## 8) Immediate Next Actions (recommended)

1. Add CI workflow for `build + test + clippy` on push/PR.
2. Add an artifact-packaging workflow for tagged releases.
3. Add example smoke tests to prevent syntax drift.
4. Open a `v1.0.x stabilization` milestone with lint-debt and DX tickets.
