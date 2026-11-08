# Development Rules — Auto Carro (working title)

These rules bind every AI agent and human working on this codebase. An agent that receives a task MUST read this file first, follow it during the task, and finish only when the Definition of Done is satisfied.

## 1. Code Quality Gates (Mandatory after every code change)

Before reporting a task complete, the agent MUST run, in this order, from the repo root:

```
cargo fmt --all -- --check     # must pass; if not, run `cargo fmt --all` and re-check
cargo clippy --all-targets -- -D warnings   # zero warnings tolerated
cargo test --all-targets        # all tests pass
```

If the task touches a Dioxus UI module, additionally verify the app builds for the target(s) it touches (at minimum `dx build --target web`; iOS/Android when the task requires it). A task is NOT complete if any gate fails — fixing the failure is part of the task.

## 2. Unit Tests (Meaningful, not decorative)

- Every pure function (calculations, parsers, date/amount logic, query builders, state reducers) MUST have unit tests in the same module (`#[cfg(test)] mod tests`) or a sibling `tests/` file.
- "Meaningful" means: test the behavior a user cares about, including **edge cases** (empty input, zero/negative amounts, month/year boundaries, leap years, zero/negative km) and **failure paths** (storage errors, API errors, malformed JSON).
- For date/deadline logic, inject a clock (e.g. a `fn now() -> DateTime` trait or parameter) so tests are deterministic. Never call `Utc::now()` inside logic under test.
- For external APIs (Mercado Livre, Supabase), define a thin trait boundary (e.g. `PartsSearchProvider`) and test the client logic against a fake. Never let unit tests hit the network.
- Bug fixes require a regression test that fails on the old code and passes on the new code.
- No `#[ignore]` without a `// TODO(owner): reason + date` comment.

## 3. Language & Style

- All application logic in **Rust** (edition 2021 or later); UI in **Dioxus**.
- Follow `rustfmt` defaults — no manual formatting debates.
- Naming: `snake_case` for functions/variables, `PascalCase` for types, `SCREAMING_SNAKE` for constants.
- Errors: use `thiserror` for library-style errors, `anyhow` at application entry points. Propagate with `?` where the type fits; add context with `.context("...")`.
- **No `unwrap()` / `expect()` in production code paths.** Only allowed in: tests, `main()`-level bootstrap, and code provably unreachable (with a `// SAFETY/NOTE:` comment explaining why).
- Money is stored and computed as **integer centavos** (`i64`). Never `f64` for money. Display formatting only at the UI layer.
- All user-facing strings in **pt-BR** (MVP language). Strings live in one module (e.g. `i18n`) to allow translation later.

## 4. Architecture Rules

- Rust workspace layout, single crate per responsibility (`app` = UI + glue, `core` = domain + logic with no UI or network imports, `net` = API clients behind traits).
- `core` MUST be platform-agnostic: it must compile and its tests must run on `wasm32-unknown-unknown`. This is the single most important rule that keeps the whole test suite fast.
- Networking and storage live behind traits in `core`; concrete implementations (Supabase client, gloo-storage, ML API client) live in `net`/`app`.
- Keep components small: a Dioxus component with more than ~150 lines should be split; business logic never lives inside a component's `render` — it lives in `core` and is passed in.
- No third-party dependency is added without a one-line justification in `decisions.md` (what it is, why, what was rejected).

## 5. Definition of Done (per task)

A task is complete only when ALL hold:
1. The task's acceptance criteria (from `plan.md`) are met.
2. `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` all pass (section 1).
3. New/changed behavior has meaningful tests (section 2).
4. `progress.md` updated: task checked off, date, one-line note on what shipped and any deviations.
5. Any architectural choice made along the way is recorded in `decisions.md`.
6. The agent's final report lists: what changed, which gates were run and their results, and what was explicitly NOT done.

## 6. Agent Working Protocol

1. Read `rules.md`, `plan.md` (task + context section), `decisions.md`, `progress.md` before writing code.
2. Follow the task's `Depends on` — if a dependency is not checked in `progress.md`, stop and report the blocker.
3. Work in small increments; run gates frequently, not once at the end.
4. If a task turns out to be underspecified or conflicts with a prior decision, stop and surface the conflict in the report — do not silently invent scope.
5. Never delete or rewrite `plan.md` / `decisions.md` / `rules.md` except via the "record a decision" step; those are owned by the project owner + lead agent.

## 7. UI/UX Rules

- Minimalist, modern, mobile-first (see `plan.md` §8 for the design language).
- Every screen needs all four states designed and implemented: **loading, empty, error, success**.
- All interactive targets ≥ 44pt; touch-friendly form layouts; keyboard/IME-friendly for pt-BR input (BRL currency field must handle comma decimals).
- No heavy assets or animations that hurt battery; icons are system/inline SVG.
- Respects system Dynamic Type / font scaling.

## 8. Security Rules

- Secrets (Supabase keys, ML tokens) live only in env vars / edge function env; never in the repo or the wasm bundle. The anon key is the only key that may appear in the client.
- All Supabase access uses Postgres Row Level Security: the client must never be able to read another user's rows. Every table gets an RLS policy tested in a migration test.
- User-uploaded documents: stored in a private bucket, served only via signed URLs scoped to the owning user.
