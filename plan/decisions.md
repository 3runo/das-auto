# Decisions Log

Critical decisions with rationale. Numbered IDs are referenced by `plan.md` and task briefs. New decisions append at the bottom of "Resolved". Agents must record any new architectural choice here before merging.

## Resolved

### D1 — Framework: Dioxus (Rust)
- **Decision:** Build the app with Dioxus; Rust workspace split into `core` (platform-agnostic domain), `net` (API clients), `app` (UI).
- **Rationale:** Single codebase for iOS/Android, type safety + memory safety for a long-lived consumer app, and `core` staying wasm32-compilable keeps the entire business logic fast-unit-testable without a device.
- **Date:** 2026-11-05.

### D2 — Mobile-first, minimalist/modern UI
- **Decision:** Mobile-first; design language fixed in `plan.md` §8 (light-first, single green accent, 8pt grid, cards, tab bar).
- **Rationale:** Vehicle interactions happen on the move; minimalist UI lowers development cost and matches the modern Brazilian consumer-app market.

### D3 — Software-only MVP (no OBD/hardware)
- **Decision:** No scanner/diagnostic features in the MVP. "Anti-scam" value = documented history + verified deadlines + parts price comparison.
- **Rationale:** Any hardware requirement is a killer for mass-market adoption in Brazil (price sensitivity). OBD can be revisited as a paid "Pro" expansion after the software base is proven.

### D4 — Marketplace strategy: link out, don't become a marketplace
- **Decision:** We search/compare Mercado Livre listings and deep-link to them; we never handle payment, shipping, or seller accounts.
- **Rationale:** Building a marketplace is a different (much larger) business; comparison + education is the MVP wedge.

### D5 — Backend: Supabase (confirmed by owner 2026-11-05)
- **Decision:** Supabase for Postgres + Auth + Storage + Edge Functions. Auth: Email + Google now (Apple Sign-In deferred per D6).
- **Rationale:** Fastest path for a mobile-first MVP: native email+Google auth, Postgres with RLS (security rules that keep multi-tenant data safe), Storage for documents, Edge Functions for the Mercado Livre proxy, all with one bill. Custom Axum/Postgres rejected for MVP speed.
- **Date:** 2026-11-05, confirmed by owner the same day.

### D6 — Apple Sign-In deferred to phase 2
- **Decision:** MVP ships Email + Google. Apple Sign-In comes later.
- **Rationale:** In a webview-based app, Sign-in with Apple needs a custom native bridge/plugin (no native button inside WKWebView/Android WebView). It's meaningful extra work for a convenience feature; Email+Google cover the bulk of Brazilian onboarding. If the App Store reviewers require it (they sometimes do for apps with third-party login), we implement the bridge in phase 2 — tracked as risk.
- **Date:** 2026-11-05, confirmed by owner 2026-11-05.

### D7 — DETRAN/Denatran: no public API → manual deadlines + deep links
- **Decision:** The app does NOT auto-verify document status with DETRAN. Users enter deadline dates once (IPVA, licenciamento, insurance); the app computes reminders, shows status, and deep-links to the official UF Detran page.
- **Rationale (researched 2026-11-05):** WSDenatran (SERPRO) requires a Denatran authorization + SERPRO contract (enterprise, cert auth); state DETRAN APIs (e.g. Detran SP's "API Detran Veículos" on openapi.api.rota.sp.gov.br) require client-id + IDP.SP OpenIDConnect and are reserved for authorized partners (insurers/banks/gov). Private apps (Olho no Carro, Check-up) operate under such contracts. Not viable for a bootstrapping consumer MVP.
- **Follow-up (phase 2+):** explore a WSDenatran or state-partner contract once we have users.
- **Date:** 2026-11-05.

### D8 — Notifications: in-app alert center only for MVP
- **Decision:** MVP has an in-app Alerts screen + home badge. OS-level push (APNs/FCM) and home-screen widgets are phase 2.
- **Rationale:** Push in a webview app needs FCM/APNs wiring (native plugin or Capacitor); in-app alerts with clear severity (expired / ≤7d / ≤30d) still deliver the core value. Acceptable UX trade-off documented in `plan.md` §9.
- **Date:** 2026-11-05, confirmed by owner 2026-11-05.

### D9 — Dioxus mobile packaging: mobile target confirmed (spike T0-1, 2026-11-07)
- **Decision:** Primary packaging = Dioxus mobile target. The spike proved `dx build --ios` builds, installs, and runs on an iOS simulator, `dx build --web` works in a real browser, and the Android leg builds, installs, launches, and renders on an emulator — including kill/restart persistence through the native bridge. The Capacitor-web fallback is NOT triggered.
- **Rationale:** Either path ships a native binary to the stores; the mobile target works end-to-end on all three tested surfaces, so switching packaging now would buy nothing.
- **Date:** 2026-11-05 (decision); iOS+Web legs recorded 2026-11-07; Android leg confirmed 2026-11-09.

#### D9 appendix — T0-1 spike report (2026-11-07)
**Environment:** macOS 27.2 (Apple Silicon), Xcode 27.2, Rust 1.99.0, Dioxus CLI 0.7.10, dioxus crate **0.8.0-alpha.3** (this combo is what T1-1 will reuse). Spike repo: `/Users/bruno/git/das-auto-spike/spike` (throwaway, last commit `4be1e9d`).

**iOS — iPhone 17 simulator (iOS 27.2, UDID 92E96A08-CBB7-4D13-B3D4-E9720B5B2373): PASS**
- Exact commands: `dx build --ios` (NO `--device`), then `xcrun simctl install booted target/dx/spike/debug/ios/Spike.app && xcrun simctl launch booted com.example.Spike`.
- **Pitfall (important):** `dx build --ios --device <udid>` builds for **device** `aarch64-apple-ios` (Mach-O platform 2) and requires a real *Apple Development* codesigning identity — it fails at "Code signing app" on a Mac with 0 valid identities. **Without `--device`** it targets the active simulator (`aarch64-apple-ios-sim`, platform 7) and ad-hoc signs automatically. Workaround for simulator work: omit `--device`. Real devices + store release will need an Apple Developer account certificate.
- Install + launch OK; app renders fully (screenshot evidence: `/tmp/spike-ios-final.png` shows all 3 cards, PDF card visible).
- **Persistence across kill/restart: PASS.** The native mobile target runs as a regular arm64 binary — it is NOT wasm — so `gloo-storage`/wasm-bindgen panic there. The spike uses a cfg split: wasm → `localStorage`, native → file at `dirs::data_dir()/dasauto-spike`. The app auto-saves a launch counter; the file in the app container read **9** before a `simctl terminate` + relaunch and **10** after; the UI showed `PERSISTED: nº 10` + `Auto-save no boot → arquivo (nativo): 10`.
- **PDF display: PASS** (with workaround). WKWebView refuses PDFs from Dioxus' custom origin; loading the asset bytes in Rust and rendering a `data:application/pdf;base64` iframe works (PDF text visible in the iOS screenshot and in the web test).
- **Camera/file input: control renders** (native "Choose File" picker inside the webview). Interaction not driven on the simulator (`simctl` has no tap API) — the identical component was exercised on the web leg below. Documented workaround if the simulator camera ever fails: plain file input without `capture`.
- Build times: cold full compile ~39s wall / 203s user; cached rebuild ~3s. App size: **26 MB** (debug `.app`).

**Web — wasm32-unknown-unknown in Chrome (CDP): PASS**
- `dx build --web` → `target/dx/spike/debug/web/public` (~38 MB uncompressed), served with `python3 -m http.server 8899`.
- localStorage round-trip: value written on load survived a full page reload and was read back (`nº 1` → reload → `nº 2`).
- File input: `DOM.setFileInputFiles` with a test PNG → status `OK: test-photo.png (70 bytes, image/png)` + base64 preview rendered (bytes read through Rust `FileData::read_bytes()`).
- PDF data-URL iframe rendered; no error states observed.

**Android — emulator pixel_6 (arm64), AVD `spike`, android-36 `google_apis;arm64-v8a` v7: PASS (2026-11-09)**
- Toolchain: NDK r27c (`27.2.12479018`), JDK Temurin 21, Gradle 9.1.0 (wrapper), AGP 8.7.0, Kotlin 2.0.20.
- Exact flow (see **D16** for why it is three steps): (1) `dx build --android` with `ANDROID_HOME`, `ANDROID_NDK_HOME`, `JAVA_HOME` set — runs the Rust/NDK cross-compile, then fails at `:app:compileDebugKotlin` (missing `lifecycle-process` in the generated project); (2) patch the generated `target/dx/spike/debug/android/app/app/build.gradle.kts` — add `implementation("androidx.lifecycle:lifecycle-process:2.8.6")`; (3) run `./gradlew :app:assembleDebug` directly in `target/dx/spike/debug/android/app` (NOT via dx — dx regenerates the project tree and wipes the patch).
- Install/launch: `adb install -r app/app/build/outputs/apk/debug/app-debug.apk` → Success; `adb shell am start -n com.example.Spike/dev.dioxus.main.MainActivity` → launches clean; `adb exec-out screencap -p` → screenshot.
- **Render: PASS.** All 3 cards fully rendered (vision-verified): title, persistence card with buttons, both camera/gallery file inputs as native pickers, PDF section. Evidence: `plan/assets/spike-android.png` (main repo).
- **Persistence across kill/restart: PASS.** UI showed `PERSISTED: nº 1` + `Auto-save no boot → arquivo (nativo): 1` — the counter value written by the previous native process was read back from the app data dir at boot, same proof pattern as the iOS leg (native file backend, not wasm).
- APK size: **72.5 MB** (debug, arm64 + .so + webview). Incremental `gradlew assembleDebug` after the Rust/NDK phase: ~21s; the failed `dx` run that produced the template error: ~7min (mostly dependency downloads over a flaky network).
- **Network pitfalls (environment, not Dioxus):** services.gradle.org wrapper download and dl.google.com maven downloads both timed out repeatedly. Fixes that worked: pre-fetch the wrapper zip with `curl --retry 30 --retry-all-errors` into `~/.gradle/wrapper/dists/gradle-9.1.0-bin/9agqghryom9wkf8r80qlhnts3/` (verify with `unzip -t`, delete stale `.lck`); global `~/.gradle/gradle.properties` with `org.gradle.jvmargs=-Dsun.net.client.defaultReadTimeout=120000 -Dsun.net.client.defaultConnectTimeout=30000 -Xmx3g`. Module cache accumulates across retries.
- **Shellcheck note:** `dx`-emitted `DX-ANDROID-EXIT:$?` after a pipeline reflects `tail`'s exit code in sh (no `PIPESTATUS`) — always read the gradle log, not the echo.

**New dependencies recorded (rules.md §4):**
- `gloo-storage 0.3` — browser `localStorage` for the wasm target (the standard wasm binding; rejected: hand-rolled wasm-bindgen localStorage calls).
- `base64 0.22` — encodes file/asset bytes into data-URLs for the PDF iframe and image previews (rejected: manual encoding; std has no base64).
- `dirs 7` — resolves the app data directory for the native (non-wasm) storage backend (rejected: hard-coded simulator container path, which changes per install).

**Known spike limitation:** native `cargo test` on the host fails by design (gloo-storage is wasm-only); the spike is throwaway — the real `core` crate in T1-1 must keep storage behind traits with a `MemoryStore` fake so host tests stay green.

### D10 — Local-first persistence with optimistic sync
- **Decision:** App reads from local cache (gloo-storage) and writes optimistically; a sync queue flushes to Supabase (source of truth) with exponential backoff + jitter; conflicts resolved last-write-wins by `updated_at`.
- **Rationale:** The core use case happens at gas stations and mechanic shops where connectivity is flaky; offline read + queued write is the expected behavior, not a nice-to-have.
- **Date:** 2026-11-05.

### D11 — Money as integer centavos, BRL only
- **Decision:** All monetary values are `i64` centavos internally; pt-BR comma/period parsing isolated in `core::money`; BRL is the only currency in the MVP.
- **Rationale:** Eliminates a whole class of float bugs and keeps the domain deterministic/testable.
- **Date:** 2026-11-05.

### D12 — Language: pt-BR only
- **Decision:** All user-facing strings in pt-BR, centralized in one `i18n` module for future translation.
- **Rationale:** MVP targets Brazil; one locale keeps scope tight while preserving i18n structure.
- **Date:** 2026-11-05.

### D13 — Working title: "Das Auto"
- **Decision:** App working title is **"Das Auto"** (owner, 2026-11-05). Local storage namespace prefix: `dasauto:`. Bundle id needs the company/CNPJ — candidate `com.<company>.dasauto`, to finalize at T3-2.
- **Date:** 2026-11-05.

### D14 — Dioxus 0.8.0-alpha.3 rsx pitfalls (verified on wasm32, T1-1)
- **Decision:** Avoid two patterns until the dioxus version is stable: (1) instantiating a component **with props inside a `match` arm inside `rsx!`** (props are not generated for that variant); (2) constructing `Component { .. }` **outside** `rsx!` as plain Rust (same missing-props error; `&'static str` props additionally fail). Mapping screen-by-state must go through a plain fn returning `Element` (e.g. `fn empty_screen(title: &str) -> Element { rsx! { ... } }`) or by switching on plain data inside `rsx!`.
- **Rationale:** Hit twice during T1-1; both break `cargo build --target wasm32-unknown-unknown` with E0559/E0277 while host compiles look fine, so only the wasm gate catches it (CI runs the web smoke build — keep it).
- **Date:** 2026-11-08.

### D15 — Conflict tie-break: local wins, plus row-compare equality (T1-2, 2026-11-09)
- **Decision:** In `resolve_conflict` (LWW by `updated_at`), when timestamps are equal the **local** copy wins (its edits are the user's most recent intent). Before comparing, both sides are canonicalized (sorted keys, nulls dropped) — equal rows mean the remote change is re-applied verbatim (no divergence), so it's treated as "local already correct" and no write-back occurs.
- **Rationale:** Equal `updated_at` is indistinguishable from a stale remote cache; surfacing that as a remote write would clobber unsent local edits. Canonicalization keeps "same data, different key order" from producing a spurious local-wins round-trip.
- **Date:** 2026-11-09.

### D16 — Dioxus 0.8.0-alpha.3 Android template bug: missing `lifecycle-process` + dx regenerates the project (T0-1, 2026-11-09)
- **Decision:** Until the dioxus version is bumped, every Android build follows this exact flow: (1) `dx build --android` — runs the Rust/NDK phase, then fails at `:app:compileDebugKotlin` with `Unresolved reference 'ProcessLifecycleOwner'`; (2) patch the generated `target/dx/<app>/debug/android/app/app/build.gradle.kts` to add `implementation("androidx.lifecycle:lifecycle-process:2.8.6")`; (3) run `./gradlew :app:assembleDebug` directly inside `target/dx/<app>/debug/android/app`. Do NOT re-run `dx build --android` after patching — **dx regenerates the entire generated Android project tree on every run and wipes local patches.**
- **Rationale:** The template's `WryActivity.kt` (copied from wry) imports `androidx.lifecycle.ProcessLifecycleOwner` + `.lifecycle` extensions, but the generated `build.gradle.kts` only declares webkit/appcompat/material — no `lifecycle-process`, so the Kotlin compile fails. Upstream context: DioxusLabs/dioxus#5878 (WryActivity lifecycle wiring) and PR #5936 (tao 0.37/wry 0.57 import reshuffle shows template deps tracking wry's Kotlin). Re-verify template deps when the dioxus pin changes — the workaround may become unnecessary (or the wrong dep).
- **Date:** 2026-11-09.

## Deferred / Open (referenced by plan.md §10)
- [ ] Bundle id: needs company/CNPJ from owner before T3-2 (candidate `com.<company>.dasauto`).
- [ ] Push/APNs plugin choice (after D9 outcome + phase 2).
