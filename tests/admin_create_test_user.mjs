// Option B — create a CONFIRMED test user via the Auth admin API.
// No email, no confirmation link, no rate limit. Requires the owner to be an
// admin (chicken-and-egg: you cannot self-promote). Run:
//   node tests/admin_create_test_user.mjs
const BASE = "BANANA";
const SECRET = "Hidden";
const ADMIN_USER = process.env.ADMIN_USER || "BANANA";
const PW = process.env.ADMIN_PW || "BANANA";
const email = process.env.BOB_EMAIL || "BANANA";

async function req(method, url, { body, headers } = {}) {
  const res = await fetch(url, {
    method, headers: { ...(headers || {}), apikey: SECRET, "Content-Type": "application/json" },
    body: body ? JSON.stringify(body) : undefined,
  });
  const txt = await res.text(); let j = null; try { j = JSON.parse(txt); } catch { j = txt; }
  return { status: res.status, txt, data: j };
}
(async () => {
  // 1. Log in as the owner using the SERVICE_ROLE key -> admin_access_token only if owner is admin.
  const l = await req("POST", BASE + "/auth/v1/token?grant_type=password", {
    method: "POST", headers: { apikey: SECRET },
    body: { email: ADMIN_USER, password: PW },
  });
  console.log("service_role login: access_token=%s admin_access_token=%s", !!l.data?.access_token, !!l.data?.admin_access_token);

  if (!l.data?.admin_access_token) {
    console.log("\nOWNER IS NOT ADMIN — admin API cannot run.\n");
    console.log("Promote the owner to admin via the Dashboard (one click):");
    console.log("  https://qjmxcszkkqnciajtebut.supabase.co/auth");
    console.log("  Auth -> Users -> click the owner -> '...' menu -> 'Make admin'.\n");
    console.log("Then re-run this script. Exiting.");
    process.exit(0);
  }

  // 2. Create a CONFIRMED user (no email) via the admin API.
  const adminTok = l.data.admin_access_token;
  const created = await req("POST", BASE + "/auth/v1/admin/users", {
    headers: { Authorization: "Bearer " + adminTok },
    body: { email, password: PW, data: { email, confirmed: true } },
  });
  console.log("admin create user: HTTP %s -> %s", created.status, JSON.stringify(created.data).slice(0, 300));

  // 3. Confirm the new user can log in (login returns a JWT).
  const login2 = await req("POST", BASE + "/auth/v1/token?grant_type=password", {
    method: "POST", headers: { apikey: SECRET },
    body: { email, password: PW },
  });
  console.log("\nnew user token: %s (login status %s)", !!login2.data?.access_token, login2.status);
  if (login2.data?.access_token) {
    console.log("=> BOB is confirmed and can log in. Set env BOB_TOKEN='" + login2.data.access_token.slice(0, 40) + "...' and run:");
    console.log("   export BOB_TOKEN=" + JSON.stringify(login2.data.access_token) + " && node tests/live_t13.mjs");
  }
})().catch((e) => console.error("FATAL", e.stack || e.message));
