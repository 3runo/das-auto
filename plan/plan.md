# Plan — Das Auto (working title)

**Version:** 2.1 (2026-11-05) — owner confirmed all open questions: Supabase backend, Apple Sign-In deferred, in-app-only alerts, working title "Das Auto".
**Owner:** Bruno. **Rules for implementers:** see `rules.md` (mandatory reading). Decisions log: `decisions.md`. Task tracking: `progress.md`.

---

## 1. Product Overview

A mobile app (iOS + Android) for Brazilian vehicle owners that gives them a **digital source of truth** for their car, so they can:

1. Stop being scammed by mechanics — a documented, timestamped history of every expense and document, exportable to the buyer.
2. Never miss a deadline — IPVA, licensing (licenciamento), and insurance reminders.
3. Buy parts smarter — search and compare part prices on Mercado Livre for *their* specific vehicle.

**Positioning:** software-only (no OBD scanner). The "anti-scam" value comes from **documented history and verified deadlines**, not live diagnostics.

### In scope (MVP)
- Auth: Email/Password + Google Sign-In. (Apple Sign-In deferred — see `decisions.md` D6.)
- Vehicle registry (multiple vehicles per user).
- Expense tracking (fuel, maintenance, insurance, tax, other) with monthly summary.
- Document vault (photos + PDFs: CRLV, insurance, etc.) with status chips and expiration tracking.
- Deadline alerts engine (in-app alert center; OS push deferred — see D8).
- Parts search & price comparison via Mercado Livre (search endpoint, proxied).
- Local-first persistence with sync to cloud.

### Out of scope (MVP)
- OBD/hardware diagnostics of any kind.
- Apple Sign-In (phase 2).
- OS-level push notifications / home-screen widgets (phase 2).
- DETRAN automatic verification (no public API exists — see D7; manual entry + deep links instead).
- Buying/purchasing flow (we link out to Mercado Livre; we are not a marketplace).
- Non-BRL currencies, non-pt-BR languages.
- Fleet/business features.

---

## 2. Technology Decisions (summary — full rationale in `decisions.md`)

| Area | Decision |
|---|---|
| UI framework | **Dioxus** (Rust), mobile target = native app wrapping a webview running WASM (`dx build --target ios/android`). Fallback if the mobile target proves unstable in the spike (T0-1): `dx build --target web` wrapped in Capacitor. |
| App architecture | Rust workspace: `core` (domain logic, platform-agnostic, fully unit-testable), `net` (API clients behind traits), `app` (Dioxus UI + glue). |
| Backend / DB | **Supabase** (Postgres + Auth + Storage + Realtime + Edge Functions). Auth providers: Email, Google (native); Apple deferred. |
| Persistence | **Local-first**: `gloo-storage` (localStorage) as on-device cache in `core`-visible traits; Supabase is the source of truth; optimistic writes with a retry queue. |
| Parts data | **Mercado Livre public search API** (`GET https://api.mercadolibre.com/sites/MLB/search?q=...`), called **from a Supabase Edge Function** (proxy + 60s cache), never directly from the app. |
| Deadline data | **No DETRAN API** (none is public/self-service). User enters due dates once; app computes reminders and deep-links to the official state Detran page. |
| Money | Integer centavos (`i64`), BRL, formatted only in UI. |
| Language | pt-BR only (MVP). |
| Auth SDK | Supabase JS auth via wasm-bindgen (session in localStorage). |

---

## 3. Architecture

```
┌──────────────────────────  Mobile app (Dioxus)  ──────────────────────────┐
│  UI components (app crate)   ←   state hooks / router                      │
│        │  domain logic calls                                              │
│  core crate: VehicleRepo, ExpenseService, AlertEngine, SyncQueue,         │
│               PartsSearchProvider (trait)                                  │
│        │  storage / net traits                                            │
│  net crate: LocalStore (gloo-storage) · SupabaseClient · MLPartsClient    │
└───────────────┬──────────────────────────────────┬─────────────────────────┘
                │ HTTPS (anon key + JWT session)   │
        ┌───────▼────────┐              ┌──────────▼──────────┐
        │   Supabase     │              │ Supabase Edge Fn    │──► Mercado Livre
        │ Postgres (RLS) │              │ ml-search proxy +   │     /sites/MLB/search
        │ Auth (Email,   │              │ 60s cache           │
        │ Google)        │              └─────────────────────┘
        │ Storage (docs) │
        └────────────────┘
```

Key properties:
- `core` compiles for `wasm32-unknown-unknown` and has the whole unit-test surface (rules.md §4).
- The app must be **usable offline** (read cached data); writes queue locally and sync when back online.
- The Mercado Livre proxy exists to (a) avoid CORS issues in the webview, (b) cache to respect rate limits (~2 req/s), (c) keep any ML token server-side.

---

## 4. Data Model (Supabase Postgres)

All tables: `id uuid pk default gen_random_uuid()`, `created_at timestamptz default now()`. Every user-scoped table has `user_id uuid references auth.users` and an RLS policy `using (auth.uid() = user_id)`.

```sql
-- vehicles
create table vehicles (
  user_id uuid not null references auth.users,
  make        text not null,           -- e.g. "Volkswagen"
  model       text not null,           -- e.g. "Gol"
  version     text,                    -- e.g. "1.6 MSI" (optional, free text)
  year        int  not null check (year between 1950 and 2030),
  plate       text not null,          -- normalized uppercase, no spaces (7 chars max, AB12C34)
  renavam     text,                    -- optional, 11 digits
  fuel_type   text not null default 'gasoline'
              check (fuel_type in ('gasoline','ethanol','flex','diesel','hybrid','electric')),
  state_uo    text not null default 'SP' -- UF of registration (2 chars), drives Detran deep-link
);
create unique index vehicles_plate_user_idx on vehicles (user_id, plate, year);

-- expenses
create table expenses (
  vehicle_id uuid not null references vehicles on delete cascade,
  user_id    uuid not null references auth.users,
  category   text not null check (category in
              ('fuel','maintenance','insurance','tax','tire','fine','other')),
  title      text not null,           -- e.g. "Troca de óleo"
  amount_cents bigint not null check (amount_cents >= 0),  -- BRL centavos
  occurred_on date not null,
  odometer_km int,                    -- optional, 0..3_000_000
  location   text,                    -- optional free text (shop name)
  notes      text,
  deleted_at timestamptz              -- soft delete
);
create index expenses_vehicle_month_idx on expenses (vehicle_id, occurred_on);

-- documents
create table documents (
  vehicle_id uuid not null references vehicles on delete cascade,
  user_id    uuid not null references auth.users,
  name       text not null,           -- e.g. "CRLV 2025"
  doc_type   text not null check (doc_type in
              ('crlv','insurance','registration','warranty','other')),
  storage_path text not null,         -- path in private Supabase Storage bucket
  valid_until date,                   -- nullable; drives expiry chip
  valid_from  date,
  deleted_at timestamptz
);

-- deadlines (the user's known dates: IPVA, licenciamento, insurance renewal, inspection)
create table deadlines (
  vehicle_id uuid not null references vehicles on delete cascade,
  user_id    uuid not null references auth.users,
  kind       text not null check (kind in ('ipva','licensing','insurance','inspection','other')),
  label      text not null,           -- e.g. "IPVA 2026"
  due_on     date not null,
  paid       bool not null default false,
  deep_link  text,                    -- URL to the official Detran page (prefilled, user-adjustable)
  deleted_at timestamptz
);

-- parts_search (saved comparisons)
create table saved_parts_searches (
  vehicle_id uuid not null references vehicles on delete cascade,
  user_id    uuid not null references auth.users,
  query      text not null,           -- the search query that was run
  snapshot   jsonb not null           -- cached top results at time of save
);
```

**Local store (gloo-storage, mirrored in app):** the same entities as JSON under namespaced keys (`dasauto:vehicles`, `dasauto:expenses:<vehicleId>`, `dasauto:syncqueue`, `dasauto:session`). Local schema mirrors the SQL types; `core` defines one Rust type per entity and (de)serialization is tested round-trip.

### Deadline semantics (AlertEngine, in `core`)
- An alert exists for each active deadline where `due_on - now() <= 30 days`.
- Severity: `expired` (due_on < today), `critical` (≤ 7 days), `upcoming` (≤ 30 days).
- Alert count for the home badge = number of `expired + critical` alerts across the user's vehicles.
- The engine is a **pure function** `fn evaluate(deadlines, now) -> Vec<Alert>` — deterministic, fully unit-tested with an injected `now`.
- MVP does NOT auto-derive IPVA/licenciamento dates per state calendar (would need per-state tables to maintain); user enters each deadline once. A *suggestion* helper (phase 2) can prefill from the known per-state tables.

---

## 5. Feature Specifications

### 5.1 Auth & Onboarding
- **Screens:** Splash → Auth (email form / "Entrar com Google") → Vehicle Wizard (required: make, model, year, plate, UF; optional: renavam, fuel) → Home.
- Email flow: sign-up sends confirmation (Supabase default); error states for invalid email / already-registered / weak password surfaced in pt-BR.
- Google flow: OAuth via Supabase; on callback, resume the wizard if the user is new.
- Session persists across app launches (Supabase session in localStorage); logout on the Profile screen.
- A user may have **0..n vehicles**; adding more is done from Home → "Adicionar veículo".

### 5.2 Expense Tracking
- **Add expense:** form fields — category (chips), title, amount (BRL field accepting comma decimal, e.g. `1.234,56`), date (default today), odometer (optional), location (optional), notes (optional). Validate: amount > 0, date ≤ today + 1 day, title non-empty.
- **List:** grouped by month (newest first), each row: title, category icon, date, formatted amount. Tapping a row → detail → edit/delete (soft delete).
- **Summary card:** current month total + previous month delta (±%); optional filter by category.
- **Aggregation logic lives in `core`** (sum by month, delta %) with unit tests incl. zero/empty months and year boundaries.
- Currency parsing/formatting: one `core::money` module; tests for pt-BR parsing quirks (`"1.234,56"`, `"1234,5"`, `"1234"`, `"R$ 1.234,56"`) and formatting.

### 5.3 Documents & Deadlines
- **Add document:** name, type (chips), validity window (optional), attach **photo (camera or gallery) or PDF**. File uploads go to a private Supabase Storage bucket `documents`; the app keeps the `storage_path` + a local thumbnail for the photo.
- **Status chips** (computed in `core`, tested): `Válido` (valid_until > today + 30d), `Expira em breve` (≤ 30d), `Vencido` (< today). Documents without `valid_until` show no chip.
- **Deadlines screen:** the same dates that drive alerts (IPVA, licenciamento, insurance, other) with a "Pagar/Pago" toggle → sets `paid` and silences the alert; "Reabrir" reactivates.
- **Detran deep link:** each deadline row has "Consultar no Detran" → opens the official site for the vehicle's UF (map of UFs → URLs in `core`, with unit tests for the table). No data is fetched (no public API — see D7).
- **Document detail:** full-size image / PDF viewer (webview), edit metadata, delete.

### 5.4 Parts Search & Comparison
- **Flow:** Parts tab → vehicle preselected → free-text query with a **smart default**: when the user types a part name (e.g. "pastilha de freio"), the client augments the query with make/model (`"pastilha de freio Volkswagen Gol 1.6"`); user can clear the augmentation.
- **Results list (top 50 by relevance/price options):** title, price (BRL), shipping (grátis / R$ x / "Frete grátis" detection), seller nickname, listing link ("Ver no Mercado Livre" → external browser via the webview's URL-open hook).
- **Sort toggles:** Relevância / Menor preço / Maior preço (client-side re-sort of the returned page; real server-side `sort` is a phase-2 enhancement of the proxy).
- **Save comparison:** "Salvar comparação" snapshots the top-N results into `saved_parts_searches` for later comparison; saved list shown under the search.
- **Proxy contract (Edge Function `ml-search`):**
  - `GET /functions/v1/ml-search?q={query}&limit={n≤50}`
  - Response: `{ results: [{id,title,price_cents: number|null,currency,shipping_free: bool, shipping_cost_cents: number|null, seller, permalink, published_date}], q: string, fetched_at: string }`
  - 60s in-memory cache keyed by `(q, limit)`; on ML error → `502 {error: "upstream"}` and the client shows its error state.
  - **ML price note:** the search endpoint returns `price` only for listings where the seller exposed it (varies by listing); the client must gracefully handle `price = null` ("Consultar").
  - The client never calls api.mercadolibre.com directly (CORS + rate limits + possible token changes).

### 5.5 Home (per vehicle)
- Vehicle card (make/model/year, plate, fuel).
- **Next deadline card** (most urgent active alert) or "Sem pendências" state.
- **Spending this month** card (total + delta).
- Badges/entry points to Expenses, Documents, Alerts, Parts, Add vehicle, Profile.

### 5.6 Profile
- User email; vehicle list (switch default vehicle); settings (notification reminders on/off — in-app only); "Sair" (logout); app version.

---

## 6. Integration & External-System Facts (researched, dated)

- **Mercado Livre API:** `GET https://api.mercadolibre.com/sites/MLB/search?q=...&limit=...&offset=...` returns JSON listings (`id`, `title`, `price`, `shipping`, `seller`, `permalink`, ...). Site ID for Brazil = `MLB`. Public search rate limits are low (~2 req/s) → hence the caching proxy. Docs: developers.mercadolivre.com.br → "Items & Searches" and "Products search". *(verified 2026-11-05)*
- **DETRAN / Denatran:** there is **no free public API** for a consumer app to check a vehicle's documents/debits:
  - *WSDenatran (SERPRO):* official vehicles/drivers/infractions API, but **requires a Denatran authorization and a SERPRO contract** (enterprise, certificate auth). Not viable for MVP.
  - *Detran SP "API Detran Veículos"* (openapi.api.rota.sp.gov.br): SP-state RENAVE API, requires client-id + IDP.SP OpenIDConnect — reserved for authorized partners (insurers, banks, gov). Same pattern in other states.
  - Private apps (Olho no Carro etc.) operate under such partnerships/contracts.
  - **MVP consequence (D7):** manual deadline entry + computed reminders + deep links to official UF Detran pages. Post-MVP option: pursue a WSDenatran/partner contract. *(verified 2026-11-05)*
- **Apple Sign-In in a webview app** is a custom-bridge effort (no native Sign-in-with-Apple button inside WKWebView/Android WebView without a native plugin). Deferred to phase 2 (D6).

---

## 7. Task Breakdown (delegation-ready)

**How to delegate:** each task below is a self-contained unit for one agent. Hand the agent: this task's block + its `Depends on` tasks' completion status + `rules.md`. The agent must finish with `rules.md` §5 Definition of Done. Tasks marked ⛔ **GATE** block all following work until verified.

### Phase 0 — Spike (feasibility gate)

**T0-1 ⛔ Dioxus mobile-target spike**
- **Goal:** prove the app can be built and installed on a real iOS device (simulator acceptable) and an Android device/emulator with the chosen target mode; measure baseline: build time, app size, localStorage persistence surviving an app restart, camera file-input behavior in the webview.
- **Do:** create a throwaway Dioxus hello-world; `dx build --target ios` and `--target android` (or the current dx mobile mechanism in the version used — document the exact commands in `decisions.md`); install on device; add a tiny localStorage round-trip test page; try `<input type="file" accept="image/*" capture>` and PDF display.
- **Fallback decision (recorded in decisions.md if triggered):** if the mobile target is broken/unusable → switch the packaging plan to `dx build --target web` + Capacitor wrapper; update the architecture section accordingly.
- **Acceptance:** install + launch works on at least one real device each platform (or emulator for Android); localStorage persists across kill/restart; camera input works (or a documented workaround chosen); a 1-page report appended to `decisions.md` (D9) with the exact commands and outcomes.
- **Depends on:** — · **Estimate:** L

### Phase 1 — Foundation

**T1-1 Repo scaffold + CI**
- **Goal:** Rust workspace (`core`, `net`, `app`) + Dioxus app skeleton + GitHub Actions CI.
- **Do:** workspace crates; Dioxus app with a router shell (Home/Expenses/Documents/Parts/Profile tabs, empty screens); `ci.yml` running `fmt --check`, `clippy --all-targets -- -D warnings`, `test`, plus `dx build --target web` as a smoke build; gitignore; README with the dev commands.
- **Acceptance:** CI green on the skeleton; tabs render; `core` has ≥ 3 trivial unit tests (deliberate, to prove the test harness works) and compiles for `wasm32-unknown-unknown` (add the wasm target to CI).
- **Depends on:** T0-1 · **Estimate:** M

**T1-2 Domain model + local storage layer**
- **Goal:** `core` types for Vehicle, Expense, Document, Deadline, SavedPartsSearch + `LocalStore` trait + gloo-storage impl + sync-queue foundation.
- **Do:** entity types with BRL centavos `i64`; serde round-trip tests (golden JSON fixtures incl. edge cases: plate normalization `ab12c34 → AB12C34`, zero amounts, nulls); `LocalStore` trait (list/get/upsert/delete per entity, all fallible) with a `MemoryStore` fake for tests and a `GlooStore` impl; `SyncQueue` trait (enqueue/drain) + in-memory impl.
- **Acceptance:** 100% of `core` storage/money/plate code covered by unit tests; `core` builds for wasm32 in CI; no UI code in this task.
- **Depends on:** T1-1 · **Estimate:** M

**T1-3 Auth + vehicle registry + Home**
- **Goal:** working Supabase auth (Email + Google), vehicle CRUD wired to Supabase with local cache, Home screen live.
- **Do:** Supabase client in `net` (session in localStorage, auto-refresh); auth screens per §5.1; vehicle wizard + list per §5.6/§5.1; migrations applied (all tables + RLS policies from §4); Home cards showing real data (empty states when none).
- **Acceptance:** sign-up → confirm → login → add 2 vehicles → logout → login restores data; RLS verified (a second test account cannot read the first's rows — documented in a manual-test note); all 4 screen states implemented; CI green.
- **Depends on:** T1-2 · **Estimate:** L

### Phase 2 — Core features (parallelizable after T1-3: T2-1, T2-2, T2-4 independent of each other)

**T2-1 Expense tracking**
- **Goal:** add/edit/delete expenses, monthly list, month summary, category filter — per §5.2.
- **Do:** `core::expenses` aggregation service (pure: totals by month, delta, category breakdown) with the full edge-case test set; UI screens + BRL amount field (comma parsing via `core::money`); optimistic local write → sync queue → Supabase.
- **Acceptance:** all §5.2 behaviors; offline write works (queue grows, flushes when online); aggregation + money tests all present and green.
- **Depends on:** T1-3 · **Estimate:** L

**T2-2 Documents + deadlines + alert engine**
- **Goal:** document upload/status, deadlines CRUD, alert center, Detran deep links — per §5.3.
- **Do:** file/PDF upload flow to private Storage bucket (signed URL for download/view); `core::alerts::evaluate` pure function + severity tests (expired/7d/30d boundaries, multiple vehicles, paid silencing); `core::detran_links` UF table + tests; Documents and Alerts screens with all 4 states.
- **Acceptance:** upload photo and PDF, view both, chip states correct at ±1 day boundaries (tests prove this), toggle "Pago" silences alert, deep link opens the correct UF URL.
- **Depends on:** T1-3 · **Estimate:** L

**T2-3 Sync layer hardening**
- **Goal:** make optimistic writes + retry queue production-grade.
- **Do:** retry with exponential backoff + jitter; idempotency (same local id → upsert, no duplicates after flaky sync); conflict policy = **last-write-wins by `updated_at`** (recorded in decisions.md); sync status indicator on Home ("Sincronizado / N pendentes").
- **Acceptance:** chaos test in CI (fake transport that drops/alters) proves no data loss and no duplicates; unit tests for backoff schedule.
- **Depends on:** T2-1 (reuses its write path) · **Estimate:** M

**T2-4 Parts search (proxy + client)**
- **Goal:** `ml-search` Edge Function + Parts screens — per §5.4.
- **Do:** Edge Function per §5.4 contract (cache, error mapping, limit clamping); `net::MLPartsClient` behind a `PartsSearchProvider` trait; fake provider with recorded fixtures for all unit tests (empty results, `price: null`, upstream 502, malformed JSON); Parts screens with smart-query augmentation, sort toggles, "Salvar comparação" (writes to `saved_parts_searches`), external link open.
- **Acceptance:** contract test of the proxy (valid q, q with accents/unicode, limit clamp, cache hit within 60s, ML-down → 502); client shows all 4 states; zero network in unit tests.
- **Depends on:** T1-3 · **Estimate:** L

### Phase 3 — Polish & store readiness

**T3-1 Polish pass**
- **Goal:** visual QA of the whole app against §8 design language; fix all missing states, empty/error/loading gaps; accessibility (44pt targets, font scaling); performance pass (first render < 3s on mid-range Android; list virtualization if needed).
- **Acceptance:** a QA checklist (screens × 4 states) 100% checked in `progress.md`; `clippy`/tests still green.
- **Depends on:** T2-1, T2-2, T2-3, T2-4 · **Estimate:** M

**T3-2 Store packaging + metadata**
- **Goal:** app ready for TestFlight / Play internal track.
- **Do:** icons/splash, bundle ids, pt-BR store metadata (name, description, screenshots — generate from the app), privacy policy URL + privacy manifest (data collected: account email, vehicle info, user-entered expenses/docs; third parties: Supabase, Mercado Livre via proxy), permissions (camera for photos).
- **Acceptance:** builds install from the store artifacts; metadata drafted in `docs/store/`.
- **Depends on:** T3-1 · **Estimate:** M

**T3-3 Beta release + feedback loop**
- **Goal:** 10–20 real users on TestFlight + Play internal track for 2 weeks.
- **Do:** seed instructions (pt-BR), a feedback form (single Supabase table `feedback`), triage notes.
- **Acceptance:** ≥ 10 installed; feedback collected; a prioritized bug list recorded in `progress.md`.
- **Depends on:** T3-2 · **Estimate:** M (mostly waiting)

---

## 8. UI/UX Design Language (minimalist / modern)

- **Theme:** light-first (dark = optional phase 2). Background `#F7F7F5`; surfaces white with 1px `#E5E5E5` borders; text primary `#111827`, secondary `#6B7280`.
- **Accent:** single accent — deep green `#15803D` (primary buttons, active tab, links). Secondary accent (alerts): amber `#D97706`, danger `#DC2626`.
- **Typography:** system font (SF/Roboto); H1 28/semibold, H2 20/semibold, body 15/regular, caption 13. Tabular numbers for all money.
- **Layout:** 8pt spacing grid; cards 16px radius, no heavy shadows (border + 1 subtle shadow max); content max-width centered; tab bar bottom (5 tabs: Carro, Gastos, Documentos, Peças, Perfil) with 24px line icons.
- **Components:** chips for categories/filters (selected = accent fill); amount input large (24px, comma-aware); deadline rows with a left severity dot; skeletons for loading; toasts (not modals) for confirmations; delete always confirmed with a sheet (not `confirm()`).
- **Empty states:** friendly pt-BR illustration-free placeholders ("Nenhum gasto neste mês" + primary CTA).
- **Reference:** Stripe/Linear mobile aesthetics — dense but airy, zero decoration without function.

---

## 9. Risks & Mitigations

| Risk | Likelihood | Mitigation |
|---|---|---|
| Dioxus mobile target immature for store release | Med | T0-1 is a hard gate; Capacitor-web fallback pre-planned (D9). |
| Apple/Google store rejection of a webview app | Low-Med | Apps with embedded webviews ship on both stores; ensure native-feel, no web-only patterns; keep a fallback plan to re-host as PWA + install. |
| Mercado Livre changes/blocks the public search API | Med | Proxy isolates it; degradation = "search unavailable" state; phase-2 alternatives (scraping-free: user-saved queries, 2º API partners) noted. |
| localStorage size limit for photos | Low | Photos upload to Storage; locally keep only small thumbnails (< 200KB). |
| In-app-only reminders weak (no push) | High (accepted) | Explicit MVP scope (D8); phase-2 FCM/APNs via Capacitor/native bridge — home screen still shows via "open app" badge only post-push. |
| pt-BR currency parsing bugs | Med | Dedicated `core::money` test suite with real-world fixtures. |
| RLS misconfiguration leaks user data | Low | Migrations include RLS + a negative-access test in CI (T1-3). |

---

## 10. Owner Confirmations (2026-11-05 — all open questions closed)

1. **Backend: Supabase — confirmed.** D5 is now a fixed decision, not a default.
2. **Apple Sign-In deferred to phase 2 — confirmed** (D6).
3. **MVP notifications: in-app alert center only, no OS push — confirmed** (D8).
4. **Working title: "Das Auto"** (D13). Bundle id still needs the company/CNPJ from the owner before T3-2; candidate `com.<company>.dasauto`. Non-blocking until T3-2.
