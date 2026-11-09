# Progress Log

## Status
- **Phase:** Phase 1 — **T1-1 done locally (all gates green, committed); T0-1 Android leg finishing (NDK downloading → emulator → install → launch).**
- **Current task:** finish T0-1 Android leg (append to D9), then **T1-2** (domain model + local storage).

### Session checkpoint 2026-11-08 (agent — T1-1 verified & committed; T0-1 Android leg unblocked)
- **T0-1 Android leg unblocked:** previous session's retry 2 actually completed — full SDK installed (platform-tools 37.0.1, platforms;android-36, build-tools;36.0.0, emulator 37.2.13, system-images;android-36;google_apis;arm64-v8a v7). **NDK was still missing** (`dx build --android` demands `ANDROID_NDK_HOME`) — installing `ndk;27.2.12479018` (r27c, latest stable). AVD `spike` (pixel_6, arm64) created; emulator booted headless (`sys.boot_completed=1`, `emulator-5554`).
- **T1-1 DONE (local gates):** workspace `core`/`net`/`app` scaffold already existed from 11-07 session (committed in `d80f8de`). This session: removed throwaway `app/src/bin/probe.rs`; fixed the wasm build break in `app/src/main.rs` (dioxus 0.8.0-alpha.3: component-with-props fails in rsx! match arms AND outside rsx! — now mapped via plain fn `empty_screen(title: &str) -> Element`; recorded as **D14** in decisions.md).
  - Gates: `cargo fmt --check` ✓ · `cargo clippy -p dasauto-core -p dasauto-net --all-targets -- -D warnings` ✓ · `cargo test` ✓ (6 core + 1 net) · `cargo check --target wasm32-unknown-unknown` (core+net) ✓ · `dx build --web` ✓.
  - Render verified via CDP on the served build: h1 "Das Auto", 5 tabs (Início/Despesas/Documentos/Peças/Perfil), clicking each tab switches content + active state.
  - **Committed:** `30a404c`. **Not done:** GitHub Actions has never run — the repo has no remote yet (owner must push to GitHub for CI verification).
- **Next steps (in order):**
  1. T0-1 Android: when NDK lands → `dx build --android` (spike repo) → `adb install` → launch `com.example.spike` → `adb exec-out screencap -p` → append Android outcome to D9 appendix (mark T0-1 complete in this file).
  2. **T1-2** in `/Users/bruno/git/das-auto`: `core` entity types (Vehicle, Expense, Document, Deadline, SavedPartsSearch) with centavos `i64` + serde round-trip tests (incl. plate normalization, zero amounts, nulls); `LocalStore` trait + `MemoryStore` fake + `GlooStore` impl (wasm-only, behind trait so host tests stay green); `SyncQueue` trait + in-memory impl.
- Gates: CI defined in `.github/workflows/ci.yml` (fmt/clippy/test/wasm32/dx-build-web) — unverifiable until repo is pushed to a GitHub remote.

### Session checkpoint 2026-11-07 (agent — T0-1 completed iOS/Web legs)
- **iOS leg DONE (PASS):** `dx build --ios` (no `--device`) → `Spike.app` (debug, 26MB, ios-sim platform 7, ad-hoc signed) → installed + launched on iPhone 17 sim (iOS 27.2, 92E96A08-…). Full D9 report in `decisions.md`.
  - Persistence proven: counter file in app container 9 → (terminate + relaunch) → 10; UI shows `PERSISTED: nº 10 · Auto-save no boot → arquivo (nativo)`. Screenshots: `/tmp/spike-ios-launch1.png`, `/tmp/spike-ios-launch2.png`, `/tmp/spike-ios-final.png` (vision-verified: all 3 cards render, PDF card visible).
  - **Key pitfall found:** `--device <udid>` makes dx build for device arm64 and demand a real Apple Development identity (this machine has 0 → signing fails). Omit `--device` for simulator builds (ad-hoc auto-sign works).
- **Web leg DONE (PASS):** `dx build --web` (38MB) served at :8899, driven via CDP: localStorage survives reload (1→2), file input `DOM.setFileInputFiles` → `OK: test-photo.png (70 bytes, image/png)` + preview, PDF data-URL iframe renders, no error states.
- **Android leg BLOCKED (network):** sdkmanager licenses hung 3h (manifest fetch 37%); retry 1 passed licenses but emulator zip corrupt ("ZipFile unknown archive"); retry 2 running in background (`/tmp/sdk_install2.log`, watch `PKG2-EXIT` / `SDK2-DONE`). `aarch64-linux-android` rustup target is installed.
- **Files updated this session:** `decisions.md` D9 finalized + spike report + dep justifications (gloo-storage, base64, dirs).
- **Next steps (in order):**
  1. Android (if SDK lands): create AVD → boot → `dx build --android` → `adb install` + launch + screencap → append outcome to D9 in `decisions.md`. If the network keeps failing, record it in D9 as an environment blocker (not a Dioxus issue — the toolchain passed iOS+web) and proceed to T1-1; the spike gate's packaging decision is already confirmed.
  2. **T1-1** in `/Users/bruno/git/das-auto`: Rust workspace `core`/`net`/`app` + Dioxus skeleton (5-tab router shell) + GitHub Actions CI (fmt/clippy/test + wasm32 check + `dx build --web` smoke). Reuse the proven combo: dx CLI 0.7.10 + dioxus 0.8.0-alpha.3 (scaffold with `dx new` in a PTY session; answers: Bare-Bones, no fullstack, no router, no tailwind, no LLM prompts, Mobile default — then convert to a workspace).
- Gates: spike is throwaway; native `cargo test` on the host fails by design in the spike (wasm-only gloo-storage) — noted in D9. In the real repo, `core` must keep storage behind traits + `MemoryStore` fake so host tests stay green.

### Session checkpoint 2026-11-07 (agent, mid-T0-1)
- Spike code was already fixed and committed (`977f6c0 fixing localstorage and pdf preview`): native iOS target is arm64 (NOT wasm), so `gloo-storage` cannot be used there; app uses cfg split — wasm → real `localStorage`, native → file in app data dir (`dirs::data_dir()/dasauto-spike`). PDF shown via `read_asset_bytes` → base64 data-URL iframe (WKWebView refuses PDFs from the dioxus custom origin). `Cargo.toml` uses **dioxus 0.8.0-alpha.3** (scaffolded by dx CLI 0.7.10) with features mobile/web/desktop; deps: gloo-storage 0.3, base64 0.22, dirs 7.
- Verified `cargo check --target wasm32-unknown-unknown --no-default-features --features web` → clean (1.4s).
- Added auto-save-on-launch (`use_effect` increments + saves counter on every launch) to main.rs — makes kill/restart persistence provable without a tap API. Not committed yet.
- iPhone 17 sim (92E96A08-CBB7-4D13-B3D4-E9720B5B2373) booted. `dx build --ios --device <udid>` started in background (bg session `proc_4e48705234bc`) — check `DX-IOS-EXIT` in its output; artifact expected at `target/aarch64-apple-ios-sim/release/spike.app` (verify actual path from output).
- Android SDK install running in background (bg session `proc_5968c7967194`, log at /tmp/sdk_install.log) — platform-tools, platforms;android-36, build-tools;36.0.0, emulator, system-images;android-36;google_apis;arm64-v8a + `rustup target add aarch64-linux-android` + JAVA_HOME/ANDROID_HOME appended to ~/.zshrc. On completion expect `SDK-INSTALL-DONE`.
- **Next steps (in order):**
  1. iOS: `xcrun simctl install booted <app>` → `xcrun simctl launch booted com.example.spike` → screenshot → `xcrun simctl terminate` → relaunch → screenshot (PERSISTENCE PROOF: counter should read nº1 then nº2). Note: first launch = nº1 (auto-save), kill/restart = nº2.
  2. `dx build --web` → serve (static dir `web` or `dx serve --release=false`) → test in real browser via CDP: localStorage round-trip (click "Salvar valor", reload page, value persists), file-input upload (DOM.setFileInputFiles with an image; expect preview + status), PDF section renders (data-URL iframe).
  3. iOS file-input/camera: try driving the Simulator with a CGEvent click (Python pyobjc Quartz) on the "GALERIA" input; if not feasible, verify via web test and record documented workaround (plain file input w/o `capture` works; simulator camera limited).
  4. Android: `avdmanager create avd -n spike -k "system-images;android-36;google_apis;arm64-v8a"` (JAVA_HOME/ANDROID_HOME from ~/.zshrc) → boot emulator (accept license, `-no-window` optional, wait for `sys.boot_completed=1` via adb) → `dx build --android` → `adb install <apk>` → launch → `adb exec-out screencap -p`.
  5. Record: build times (time output), app sizes (`du -sh` on .app/.apk), all outcomes → write **D9 report** appended to `/Users/bruno/git/das-auto/plan/decisions.md`; record gloo-storage/base64/dirs deps (one line each); check T0-1 in progress.md.
  6. Then **T1-1**: create Rust workspace `core`/`net`/`app` + Dioxus skeleton + CI at `/Users/bruno/git/das-auto` (use same dioxus 0.8.0-alpha.3 + dx CLI 0.7.10 combo proven in spike).
- Gates: spike is throwaway; native `cargo test` on host fails by design (wasm-only gloo-storage) — note in D9, don't "fix".

## Delegation order (see plan.md §7 for full task specs)
1. T0-1 spike (gate) → 2. T1-1 scaffold/CI → 3. T1-2 domain+storage → 4. T1-3 auth+vehicles+Home → 5. T2-1/T2-2/T2-4 (parallelizable) → 6. T2-3 sync → 7. T3-1 polish → 8. T3-2 store → 9. T3-3 beta.

## Completed
- [x] 2026-11-05 — Initial plan v1 (overview-level) — superseded.
- [x] 2026-11-05 — Plan v2: rewritten as delegation-ready plan (task breakdown, data model, feature specs, UI language, risks).
- [x] 2026-11-05 — `decisions.md`: D1–D12 resolved (backend default Supabase, Apple SSO deferred, DETRAN no-API finding documented, Dioxus packaging strategy with fallback).
- [x] 2026-11-05 — `rules.md`: agent protocol, quality gates (fmt/clippy/test), mandatory meaningful unit tests, architecture and security rules.
- [x] 2026-11-05 — Researched DETRAN/Denatran API landscape (no public self-service API) and Mercado Livre search API details; recorded in plan.md §6 + D7.
- [x] 2026-11-05 — Owner answered all open questions: Supabase backend confirmed, Apple SSO deferred, in-app-only alerts, working title "Das Auto" (D13). Plan bumped to v2.1; §10 now "Owner Confirmations".
- [x] 2026-11-08 — **T1-1 Repo scaffold + CI** (local gates all green: fmt/clippy/test/wasm32/dx-build-web; 5-tab skeleton render verified via CDP; dioxus rsx pitfall recorded as D14; commit `30a404c`). Caveat: GitHub Actions not yet run — no GitHub remote on the repo yet (owner action); CI file is in place per plan.

## In Progress
- [ ] **T0-1 Dioxus mobile-target spike** — iOS leg PASS + Web leg PASS (2026-11-07); **Android leg in progress** (2026-11-08): SDK + AVD + emulator ready, NDK r27c downloading → build → install → screencap → D9 appendix.

## T0-1 HANDOFF — for the agent resuming this task

### What already exists
- **Spike app:** `/Users/bruno/git/das-auto-spike/spike` (throwaway, *outside* the main repo `/Users/bruno/git/das-auto`; T1-1 will create the real workspace there).
  - Created with `dx new spike --name spike --vcs none` (Dioxus CLI 0.7.10). **Pitfall:** `dx new` requires a TTY — run it in a background PTY session and answer the prompts: subtemplate=**Bare-Bones**, fullstack=**false**, router=**false**, tailwind=**false**, LLM prompts=**false**, default platform=**Mobile**.
  - `src/main.rs` = single 3-section test screen: (1) **localStorage round-trip** via gloo-storage (key `dasauto:spike:counter`, "Salvar valor" / "Reler" buttons, shows persisted value big); (2) **camera/file input** — two `<input type="file" accept="image/*">` (one with `capture="environment"`, one without), `FileData::read_bytes()` → base64 data-URL image preview + status text; (3) **PDF display** — `<iframe>` pointing at local asset `assets/test.pdf`.
  - `assets/test.pdf` = 608-byte minimal valid PDF (text "Das Auto - PDF display test 12345"), generated with a Python one-liner.
  - `assets/main.css` = light theme per plan.md §8. `Cargo.toml`: dioxus 0.7.1 (default features `["mobile"]`, plus `web`/`desktop` features), **gloo-storage 0.3, base64 0.22** (both new deps — need one-line justifications in decisions.md per rules.md §4).
- **Nothing built or run successfully yet:** no simulator/emulator runs, no D9 report written, no env persisted in `~/.zshrc`.

### Environment (verified 2026-11-05; macOS 27.2, Apple Silicon Mac16,1, ~52GB free)
- Rust 1.99.0 / cargo 1.99.0. Installed rustup targets: `wasm32-unknown-unknown`, `aarch64-apple-ios`, `aarch64-apple-ios-sim` (+darwin). **`aarch64-linux-android` NOT installed yet.**
- Dioxus CLI 0.7.10 at `/Users/bruno/.cargo/bin/dx`. `dx build` flags: `--ios`, `--android`, `--web`, `--device <name>`.
- **Xcode 27.2**; iOS 27.2 simulators available — recommended: **iPhone 17 (87FD4002-5CDF-4C23-96D1-2B74A4B0B2CB)** (also iPhone 18 Pro, iPhone Air, iPhone 17e). **No physical iOS device connected** (task goal allows simulator for iOS; acceptance wording prefers real device where available).
- `dx doctor`: iOS toolchain OK; it reports "Simulator: not found" because it looks for `simctl` on PATH — `xcrun simctl` works; if `dx build --ios` complains, put `/Applications/Xcode.app/Contents/Developer/usr/bin` on PATH or symlink `simctl`.
- Homebrew 7.0.11. **Pitfall: `brew install --cask temurin` fails (needs sudo password).**

### Android toolchain state (partially installed, no sudo needed)
- **JDK ✓ (ready):** Temurin 21 extracted at `~/.jdks/temurin-21/Contents/Home` (verified: `~/.jdks/temurin-21/Contents/Home/bin/java` → OpenJDK 21.0.13). Tarball still at `/tmp/jdk21.tar.gz`.
- **Android SDK ◐:** `~/Library/Android/sdk/cmdline-tools/latest` ✓ (sdkmanager works). **NOT installed:** platform-tools, platforms;android-36, build-tools;36.0.0, emulator, system images.
- Finish with:
  ```
  export JAVA_HOME=~/.jdks/temurin-21/Contents/Home
  export ANDROID_HOME=~/Library/Android/sdk
  echo y | $ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager --licenses
  $ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager "platform-tools" "platforms;android-36" "build-tools;36.0.0" "emulator" "system-images;android-36;google_apis;arm64-v8a"
  rustup target add aarch64-linux-android
  ```
  Then persist JAVA_HOME/ANDROID_HOME/PATH in `~/.zshrc` (current .zshrc has nothing Android-related).
- Emulator AVD not created yet: `avdmanager create avd -n spike -k "system-images;android-36;google_apis;arm64-v8a"`.

### Compile status: 5 known errors, none fixed yet
Re-run to see full text: `cd /Users/bruno/git/das-auto-spike/spike && cargo check --target wasm32-unknown-unknown --no-default-features --features web` (dx build --web compiles with only the `web` feature).
1. `src/main.rs:62` — *"unmatched closing '}' in format string"*: the rsx text `"Dioxus {env!("CARGO_PKG_VERSION")} · ..."` — in rsx, `{...}` inside a string literal is a format placeholder. Fix: `let version = env!("CARGO_PKG_VERSION");` then `Dioxus {version} · ...` (or drop the version).
2. `src/main.rs:16` E0308 — `v.parse().ok()` inference in `read_counter()`; try explicit `v.parse::<u64>().ok()`.
3. `src/main.rs:121` E0599 — `base64::engine::general_purpose::STANDARD.encode(...)` needs `use base64::Engine;` (trait method).
4. `src/main.rs:130/131` E0277 — `signal.read()` returns `GenerationalRef` which doesn't implement `IntoDynNode`/`IntoAttributeValue`. Fix: render the `Signal` directly (`{file_status}`, `{preview}`) or call `.to_string()` (e.g. `src: {preview.read().to_string()}`).

### Suggested execution order
1. Fix the 4 compile issues → `cargo check --target wasm32-unknown-unknown --no-default-features --features web` clean.
2. `dx build --web` (smoke build; also confirms asset URL mapping — verify `/assets/main.css` and `test.pdf` resolve in the bundle).
3. **iOS:** `xcrun simctl boot "iPhone 17"` → `dx build --ios` → install (`xcrun simctl install booted <app>`) → launch → `xcrun simctl io booted screenshot <png>`.
   - **Persistence proof across kill/restart:** `simctl` has no tap API — easiest: make the app auto-increment + save the counter on startup, then terminate + relaunch and screenshot the increased value (keep the manual buttons for real-device testing).
   - Verify PDF section renders (screenshot). Try camera input; simulator camera is limited — if it fails, record the documented workaround (plain file input without `capture` is the fallback).
   - Record build time + app size (`du -sh` on the .app/.ipa).
4. **Android:** finish SDK install (above) → create AVD → start emulator → `dx build --android` → `adb install` → launch package → `adb exec-out screencap -p` screenshot.
5. **Write D9 report** appended to `plan/decisions.md` (one page): exact commands, build times, app sizes, localStorage/camera/PDF outcomes, and the packaging decision (keep Dioxus mobile target vs fallback `dx build --target web` + Capacitor — if fallback, also update plan.md §3 architecture section).
6. **Update `plan/progress.md`** (check T0-1 off with date + one-line note) and record the `gloo-storage`/`base64` deps in `plan/decisions.md` (one-line justification each, rules.md §4).
7. Gates: spike is throwaway and wasm-only — `gloo-storage` does not compile on the host target, so native `cargo test` fails by design for this crate; note that explicitly in the final report instead of "fixing" it.

## Next Steps
- [x] 2026-11-05 — Owner confirmed open questions (plan.md §10). Nothing left blocking delegation except company/CNPJ at T3-2.
- [x] 2026-11-08 — T1-1 scaffold + local gates + commit (see checkpoint above).
- [ ] T0-1 Android leg (NDK download → build → install → screencap → D9 appendix; details in 2026-11-08 checkpoint).
- [ ] T1-2 domain model + local storage (details in 2026-11-08 checkpoint).
- [ ] Owner: push repo to a GitHub remote so the CI in `.github/workflows/ci.yml` can run (non-blocking locally).

## Issues / Notes
- Earlier `rules.md` write landed in a stray path `/Users/bruno/s/Users/bruno/plans/rules.md` (malformed path from a prior session). The correct file now lives at `/Users/bruno/plans/rules.md`. The stray dir `/Users/bruno/s` is junk (contains only that file + .DS_Store) and should be deleted by the owner.
- 2026-11-05 — `brew install --cask temurin` requires sudo (password prompt not available in agent runs); use the Adoptium tarball API into `~/.jdks` instead (done, see above).
- 2026-11-05 — `dx new` is interactive and fails with "not a terminal" in plain terminal sessions; use a PTY background session + submit answers (answers recorded above).
- 2026-11-05 — `dx build --web` compiles with `--cfg feature="web"` only (default features off) — the crate's `web` feature must exist for web builds (it does).
