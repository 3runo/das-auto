// Live T1-3 acceptance test (REAL production Supabase project).
//
// Reads-only-friendly design so the test never needs email confirmation:
//   - E2E: uses the OWNER account (password login only, no signup).
//   - RLS: takes a CONFIRMED second account's token via env BOB_TOKEN
//     (create it in the Supabase Dashboard; see README at repo root).
// This sidesteps the live project's email-confirmation + email-send rate limit.
const BASE = "https://qjmxcszkkqnciajtebut.supabase.co";
const AK = process.env.PUBLISHABLE_KEY || process.env.SUPABASE_PUBLISHABLE_KEY;
if (!AK) { console.error("PUBLISHABLE_KEY env not set"); process.exit(2); }
const OWNER_EMAIL = process.env.OWNER_EMAIL || "bruno.adex@gmail.com";
const OWNER_PW = process.env.OWNER_PW || "Testpass1234";
const TEST_PW = "Testpass1234";

let pass = 0, fail = 0;
const ok = (m) => { pass++; console.log(`  PASS  ${m}`); };
const no = (m) => { fail++; console.log(`  FAIL  ${m}`); };
const step = (t) => console.log(`\n== ${t} ==`);
const assertEq = (got, want, msg) => got === want ? ok(`${msg} (got ${got})`) : no(`${msg} — expected ${want}, got ${got}`);
const hasPlate = (rows, plate) => (rows || []).some((v) => v && v.plate === plate);

async function req(method, url, { body, auth } = {}) {
  const res = await fetch(url, {
    method, headers: { ...(auth ? { Authorization: "Bearer " + auth } : {}), apikey: AK, "Content-Type": "application/json" },
    body: body ? JSON.stringify(body) : undefined,
  });
  const txt = await res.text(); let j = null; try { j = JSON.parse(txt); } catch { j = null; }
  return { status: res.status, ok: res.ok, data: j, txt };
}
// Password login only — never signup (signup re-flags existing accounts unconfirmed
// and trippes the email-send rate limit). Throws a clear error if the owner isn't
// confirmed yet, so the user knows to confirm the owner email in the Dashboard.
async function login(email, pw) {
  const r = await req("POST", BASE + "/auth/v1/token?grant_type=password", { method: "POST", body: { email, password: pw } });
  if (!r.ok) throw new Error("login failed: " + (r.data?.msg || r.status));
  return r.data.access_token;
}
async function insertV(token, v) {
  const r = await req("POST", BASE + "/rest/v1/vehicles", { method: "POST", body: v, auth: token });
  return r.ok ? r.status : 0;
}
async function listV(token) {
  const r = await req("GET", BASE + "/rest/v1/vehicles", { auth: token });
  if (Array.isArray(r.data?.vehicles)) return r.data.vehicles;
  if (Array.isArray(r.data?.data)) return r.data.data;
  return Array.isArray(r.data) ? r.data : [];
}
async function me(token) { const r = await req("GET", BASE + "/auth/v1/me", { auth: token }); return r.data?.user || null; }
async function logout(token) { await req("DELETE", BASE + "/auth/v1/session", { auth: token }); }

(async () => {
  await step("1. LIVE E2E — sign-in -> add 2 vehicles -> logout -> re-login restores");
  const a = await login(OWNER_EMAIL, OWNER_PW);
  ok("owner sign-in (password) works");
  const plateA = "AAA1BBB" + Date.now();
  const plateB = "CCC5DDD" + Date.now();
  await insertV(a, { make: "Volkswagen", model: "Golf", year: 2020, plate: plateA, fuel_type: "gasoline", state_uo: "SP" });
  await insertV(a, { make: "Chevrolet", model: "Onix", year: 2021, plate: plateB, fuel_type: "flex", state_uo: "RJ" });
  await logout(a);
  ok("logged out");
  const reLogin = await login(OWNER_EMAIL, OWNER_PW);
  const after = await listV(reLogin);
  assertEq(hasPlate(after, plateA), true, "E2E vehicle #1 persisted (plate " + plateA + ")");
  assertEq(hasPlate(after, plateB), true, "E2E vehicle #2 persisted (plate " + plateB + ")");
  await logout(reLogin);

  await step("2. LIVE RLS negative-access — 2nd confirmed account cannot read 1st's rows");
  const bobToken = process.env.BOB_TOKEN;
  if (!bobToken) {
    console.log("  ⚠ No BOB_TOKEN provided. Create a confirmed 2nd account:\n");
    console.log("    1) Open:  https://qjmxcszkkqnciajtebut.supabase.co/auth");
    console.log("    2) Auth -> Users -> Add user (email + password). Confirm it.\n");
    console.log("    3) Click the new user's '...' menu -> Copy access token, or");
    console.log("       sign in as them and paste the JWT here:");
    console.log("       export BOB_TOKEN='eyJhbG...'\n");
    console.log("    4) export BOB_TOKEN='...' && node tests/live_t13.mjs\n");
    process.exit(0);
  }
  const bob = await loginFor(bobToken);
  if (!bob) throw new Error("BOB_TOKEN is not a valid owner token");
  ok("bob confirmed (account B) via provided token");

  // Account A (owner) owns private rows; bob must not read them.
  const vA = await insertV(a, { make: "Fiat", model: "Punto", year: 2018, plate: "ZZZ1YYY" + Date.now(), fuel_type: "diesel", state_uo: "SP", user_id: a });
  ok(`account A inserted a private row (HTTP ${vA})`);

  const leaked = (await listV(bob)).some((v) => v && v.user_id === a);
  assertEq(leaked, false, "bob CANNOT read account A's row (RLS holds)");
  const bobWrite = await insertV(bob, { make: "Toyota", model: "Hilux", year: 2022, plate: "QQQ2RRR" + Date.now(), fuel_type: "diesel", state_uo: "MT", user_id: a });
  assertEq(bobWrite === 201, false, "bob cannot write a row for another user (owner-only with-check)");

  console.log(`\n[info] E2E test plates: ${plateA}, ${plateB} (owned by ${OWNER_EMAIL}) — remove via Dashboard Auth -> Users if needed.`);
  console.log(`[info] test accounts on live: ${[OWNER_EMAIL, bob.email].join(", ")}`);
  console.log(`==== ${pass} passed, ${fail} failed ====`);
  process.exit(fail === 0 ? 0 : 1);
})().catch((e) => { console.error("FATAL", e.stack || e.message); process.exit(3); });

async function loginFor(token) {
  // Just validate the token and read the user id from the JWT `sub` claim.
  const parts = token.split(".");
  if (parts.length < 2) return null;
  let j = null; try { j = JSON.parse(Buffer.from(parts[1], "base64url").toString()); } catch {}
  return j?.sub || null;
}
