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
- **Decision:** Primary packaging = Dioxus mobile target. The spike proved `dx build --ios` builds, installs, and runs on an iOS simulator, and `dx build --web` works in a real browser. The Capacitor-web fallback is NOT triggered. (Android leg was still blocked on the SDK download at report time — see report below; it does not change the decision, it only delays the Android confirmation.)
- **Rationale:** Either path ships a native binary to the stores; the mobile target works end-to-end for the parts that were testable, so switching packaging now would buy nothing.
- **Date:** 2026-11-05 (decision), spike outcome recorded 2026-11-07.

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

**Android: PENDING (network-blocked, retry in progress)**
- `sdkmanager --licenses` hung 3h at "Fetching remote repository 37%" (network to dl.google.com flaky); a retry passed licenses but the **emulator zip download came back corrupt** ("Error on ZipFile unknown archive"). Second retry running in background; `aarch64-linux-android` rustup target installed in the meantime. When the SDK lands: `avdmanager create avd -n spike -k "system-images;android-36;google_apis;arm64-v8a"` → boot → `dx build --android` → `adb install` → launch → `adb exec-out screencap -p`, and append the outcome here.

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

## Deferred / Open (referenced by plan.md §10)
- [ ] Bundle id: needs company/CNPJ from owner before T3-2 (candidate `com.<company>.dasauto`).
- [ ] Push/APNs plugin choice (after D9 outcome + phase 2).
