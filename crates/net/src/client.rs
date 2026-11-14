//! Supabase backend client (plan.md §2, D17).
//!
//! Thin, explicit REST client over GoTrue (`/auth/v1`) and PostgREST
//! (`/rest/v1`). The `supabase` crate on the registry is a 0.0.0
//! placeholder (not maintained), and the plan's Supabase-JS-via-wasm
//! fallback is heavier and harder to test than the stable REST endpoints
//! this module implements.
//!
//! Security (rules.md §8):
//! - the publishable API key is the only credential this client holds; it is
//!   baked in at build time and never stored or logged;
//! - the session (user tokens) is persisted via `LocalStore` under
//!   `dasauto:session` and auto-refreshed ~60 s before expiry (D17);
//! - ownership is enforced by Postgres RLS server-side; this client never
//!   filters rows by `user_id` client-side.
//!
//! Error policy (rules.md §3): no `unwrap`/`expect` on production paths —
//! malformed JSON, store errors, and transport failures all map to typed
//! errors the UI renders as pt-BR messages.

use std::sync::{Arc, Mutex};

use dasauto_core::api::{
    AuthError, AuthService, BoxFuture, RepoError, SignUpOutcome, VehicleService,
};
use dasauto_core::entities::Vehicle;
use dasauto_core::session::{now_epoch_secs, Profile, Session};
use dasauto_core::store::LocalStore;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

/// `LocalStore` key for the persisted session.
pub const SESSION_KEY: &str = "dasauto:session";

/// Refresh the access token this many seconds before it actually expires.
const REFRESH_LEEWAY_SECS: i64 = 60;

/// Supabase REST client implementing both service contracts.
#[derive(Clone)]
pub struct SupabaseClient {
    base_url: String,
    api_key: String,
    http: reqwest::Client,
    store: Arc<dyn LocalStore>,
    session: Arc<Mutex<Option<Session>>>,
}

impl SupabaseClient {
    /// Build a client. `None`/empty URL or key means "not configured" — the
    /// client still works for session inspection but every network call
    /// returns `Unconfigured` (rules.md §3: no panic).
    pub fn new(
        base_url: Option<String>,
        api_key: Option<String>,
        store: Arc<dyn LocalStore>,
    ) -> Self {
        let http = reqwest::Client::builder()
            .build()
            .unwrap_or_default();
        Self {
            base_url: base_url.unwrap_or_default(),
            api_key: api_key.unwrap_or_default(),
            http,
            store,
            session: Arc::new(Mutex::new(None)),
        }
    }

    /// True when a backend URL and publishable key were provided at build.
    pub fn is_configured(&self) -> bool {
        !self.base_url.is_empty() && !self.api_key.is_empty()
    }

    fn base(&self) -> &str {
        self.base_url.trim_end_matches('/')
    }

    fn lock_session(&self) -> Option<Session> {
        self.session.lock().ok().and_then(|g| g.clone())
    }

    fn load_persisted(&self) -> Option<Session> {
        self.store
            .get_raw(SESSION_KEY)
            .ok()
            .flatten()
            .and_then(|raw| serde_json::from_str::<Session>(&raw).ok())
    }

    fn set_session(&self, s: Option<Session>) {
        if let Ok(mut g) = self.session.lock() {
            *g = s.clone();
        }
        match &s {
            Some(sess) => {
                if let Ok(json) = serde_json::to_string(sess) {
                    // Best-effort persistence: a full/locked localStorage
                    // must not take the in-memory session down.
                    let _ = self.store.set_raw(SESSION_KEY, &json);
                }
            }
            None => {
                let _ = self.store.remove_raw(SESSION_KEY);
            }
        }
    }

    /// A usable access token: refreshes it when within the expiry leeway.
    async fn valid_token(&self) -> Result<String, AuthError> {
        if !self.is_configured() {
            return Err(AuthError::Unconfigured);
        }
        let session = self.stored_session().ok_or(AuthError::NoSession)?;
        if session.needs_refresh(now_epoch_secs(), REFRESH_LEEWAY_SECS) {
            let refreshed = self
                .refresh_tokens(&session.refresh_token)
                .await
                .map_err(|_| AuthError::NoSession)?;
            let token = refreshed.access_token.clone();
            self.set_session(Some(refreshed));
            Ok(token)
        } else {
            Ok(session.access_token)
        }
    }

    async fn refresh_tokens(&self, refresh_token: &str) -> Result<Session, AuthError> {
        let url = format!("{}/auth/v1/token?grant_type=refresh_token", self.base());
        let resp = self
            .http
            .post(url)
            .header("apikey", &self.api_key)
            .json(&json!({ "refresh_token": refresh_token }))
            .send()
            .await
            .map_err(|_| AuthError::Network)?;
        let status = resp.status().as_u16();
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        if !(200..300).contains(&status) {
            return Err(classify_auth_error(status, &body));
        }
        parse_session(&body, "").ok_or(AuthError::Server {
            status,
            message: "malformed GoTrue session".into(),
        })
    }
}

impl AuthService for SupabaseClient {
    fn sign_up_email<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
    ) -> BoxFuture<'a, Result<SignUpOutcome, AuthError>> {
        Box::pin(async move {
            if !self.is_configured() {
                return Err(AuthError::Unconfigured);
            }
            let url = format!("{}/auth/v1/signup", self.base());
            let resp = self
                .http
                .post(url)
                .header("apikey", &self.api_key)
                .json(&json!({ "email": email, "password": password }))
                .send()
                .await
                .map_err(|_| AuthError::Network)?;
            let status = resp.status().as_u16();
            let body: Value = resp.json().await.unwrap_or(Value::Null);
            if !(200..300).contains(&status) {
                return Err(classify_auth_error(status, &body));
            }
            // GoTrue returns the session when email is auto-confirmed, and
            // the bare user object (no access_token) when confirmation is
            // required first.
            match parse_session(&body, email) {
                Some(s) => {
                    self.set_session(Some(s.clone()));
                    Ok(SignUpOutcome::Authenticated(s))
                }
                None => Ok(SignUpOutcome::ConfirmationSent),
            }
        })
    }

    fn sign_in_email<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
    ) -> BoxFuture<'a, Result<Session, AuthError>> {
        Box::pin(async move {
            if !self.is_configured() {
                return Err(AuthError::Unconfigured);
            }
            let url = format!(
                "{}/auth/v1/token?grant_type=password",
                self.base()
            );
            let resp = self
                .http
                .post(url)
                .header("apikey", &self.api_key)
                .json(&json!({ "email": email, "password": password }))
                .send()
                .await
                .map_err(|_| AuthError::Network)?;
            let status = resp.status().as_u16();
            let body: Value = resp.json().await.unwrap_or(Value::Null);
            if !(200..300).contains(&status) {
                return Err(classify_auth_error(status, &body));
            }
            let session = parse_session(&body, email).ok_or(AuthError::Server {
                status,
                message: "malformed GoTrue session".into(),
            })?;
            self.set_session(Some(session.clone()));
            Ok(session)
        })
    }

    fn restore_session(&self) -> BoxFuture<'_, Result<Option<Session>, AuthError>> {
        Box::pin(async move {
            let mut session = self.stored_session();
            if let Some(stored) = &mut session {
                if stored.needs_refresh(now_epoch_secs(), REFRESH_LEEWAY_SECS) {
                    // Best-effort refresh: offline restore keeps the stored
                    // session (vehicle calls will surface the real error).
                    let refresh_token = stored.refresh_token.clone();
                    if let Ok(fresh) = self.refresh_tokens(&refresh_token).await {
                        self.set_session(Some(fresh.clone()));
                        *stored = fresh;
                    }
                }
            }
            Ok(session)
        })
    }

    fn sign_out(&self) -> BoxFuture<'_, Result<(), AuthError>> {
        Box::pin(async move {
            // Best-effort remote revocation, then always clear local state —
            // the local sign-out must work offline (rules.md §7).
            if let Some(session) = self.stored_session() {
                if self.is_configured() {
                    let url = format!("{}/auth/v1/logout", self.base());
                    let _ = self
                        .http
                        .post(url)
                        .header("apikey", &self.api_key)
                        .bearer_auth(&session.access_token)
                        .send()
                        .await;
                }
            }
            self.set_session(None);
            Ok(())
        })
    }

    fn exchange_pkce(&self, code: &str, verifier: &str) -> BoxFuture<'_, Result<Session, AuthError>> {
        // Owned copies: `BoxFuture<'_>` is bound to `&self` only, so the boxed
        // future must not capture the caller's `&str` references.
        let code = code.to_string();
        let verifier = verifier.to_string();
        Box::pin(async move {
            if !self.is_configured() {
                return Err(AuthError::Unconfigured);
            }
            let url = format!("{}/auth/v1/token?grant_type=pkce", self.base());
            let resp = self
                .http
                .post(url)
                .header("apikey", &self.api_key)
                .json(&json!({ "code": code, "code_verifier": verifier }))
                .send()
                .await
                .map_err(|_| AuthError::Network)?;
            let status = resp.status().as_u16();
            let body: Value = resp.json().await.unwrap_or(Value::Null);
            if !(200..300).contains(&status) {
                return Err(classify_auth_error(status, &body));
            }
            let session = parse_session(&body, "").ok_or(AuthError::Server {
                status,
                message: "malformed GoTrue session".into(),
            })?;
            self.set_session(Some(session.clone()));
            Ok(session)
        })
    }

    fn stored_session(&self) -> Option<Session> {
        self.lock_session().or_else(|| self.load_persisted())
    }

    fn current_profile(&self) -> Option<Profile> {
        self.stored_session().map(|s| s.profile())
    }
}

impl VehicleService for SupabaseClient {
    fn list(&self) -> BoxFuture<'_, Result<Vec<Vehicle>, RepoError>> {
        Box::pin(async move {
            let token = self
                .valid_token()
                .await
                .map_err(RepoError::from)?;
            let url = format!(
                "{}/rest/v1/vehicles?select=*&order=created_at.asc",
                self.base()
            );
            let resp = self
                .http
                .get(url)
                .header("apikey", &self.api_key)
                .bearer_auth(&token)
                .send()
                .await
                .map_err(|_| RepoError::Network)?;
            read_json(resp).await
        })
    }

    fn create(&self, vehicle: Vehicle) -> BoxFuture<'_, Result<Vehicle, RepoError>> {
        Box::pin(async move {
            let token = self.valid_token().await.map_err(RepoError::from)?;
            let url = format!("{}/rest/v1/vehicles", self.base());
            let resp = self
                .http
                .post(url)
                .header("apikey", &self.api_key)
                .bearer_auth(&token)
                .header("Prefer", "return=representation")
                .json(&vehicle_payload(&vehicle))
                .send()
                .await
                .map_err(|_| RepoError::Network)?;
            read_json(resp).await
        })
    }

    fn remove(&self, id: dasauto_core::Uuid) -> BoxFuture<'_, Result<(), RepoError>> {
        Box::pin(async move {
            let token = self.valid_token().await.map_err(RepoError::from)?;
            let url = format!("{}/rest/v1/vehicles?id=eq.{id}", self.base());
            let resp = self
                .http
                .delete(url)
                .header("apikey", &self.api_key)
                .bearer_auth(&token)
                .send()
                .await
                .map_err(|_| RepoError::Network)?;
            let status = resp.status().as_u16();
            if !(200..300).contains(&status) {
                let body: Value = resp.json().await.unwrap_or(Value::Null);
                return Err(postgrest_error(status, &body));
            }
            Ok(())
        })
    }
}

/// Parse a 2xx JSON body, or a typed error otherwise.
async fn read_json<T: DeserializeOwned + Send>(
    resp: reqwest::Response,
) -> Result<T, RepoError> {
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        return Err(postgrest_error(status, &body));
    }
    resp.json::<T>()
        .await
        .map_err(|_| RepoError::Server {
            status,
            message: "malformed response".into(),
        })
}

/// Map a PostgREST error body to `RepoError` (message kept for logs only —
/// the UI renders a generic pt-BR message; rules.md §6).
fn postgrest_error(status: u16, body: &Value) -> RepoError {
    let message = body
        .get("message")
        .or_else(|| body.get("msg"))
        .and_then(Value::as_str)
        .unwrap_or("unknown error")
        .to_string();
    RepoError::Server { status, message }
}

/// Extract the payload sent to PostgREST for a new vehicle.
///
/// `user_id` is intentionally omitted (the server sets it from the JWT via
/// trigger/RLS — client-side filtering is forbidden, rules.md §8), as are
/// `created_at`/`updated_at` (server-managed).
fn vehicle_payload(v: &Vehicle) -> Value {
    json!({
        "id": v.id,
        "make": v.make,
        "model": v.model,
        "version": v.version,
        "year": v.year,
        "plate": v.plate,
        "renavam": v.renavam,
        "fuel_type": v.fuel_type,
        "state_uo": v.state_uo,
    })
}

/// Parse a GoTrue session object (`access_token` + `refresh_token` + `user`).
///
/// Tolerant of field drift between GoTrue versions: `user.email` falls back
/// to `email_hint`; `expires_at` falls back to `expires_in + now`; a body
/// without `access_token` (e.g. signup pending email confirmation) is `None`.
fn parse_session(body: &Value, email_hint: &str) -> Option<Session> {
    let access_token = body.get("access_token")?.as_str()?.to_string();
    let refresh_token = body.get("refresh_token")?.as_str()?.to_string();
    let user = body.get("user")?;
    let user_id = user.get("id")?.as_str()?.to_string();
    let email = user
        .get("email")
        .and_then(Value::as_str)
        .unwrap_or(email_hint)
        .to_string();
    let expires_at = body
        .get("expires_at")
        .and_then(Value::as_i64)
        .or_else(|| {
            body.get("expires_in")
                .and_then(Value::as_i64)
                .map(|in_secs| now_epoch_secs() + in_secs)
        })?;
    Some(Session {
        access_token,
        refresh_token,
        expires_at,
        user_id,
        email,
    })
}

/// Classify a GoTrue error response.
///
/// GoTrue returns `{"error": "...", "error_code": "..."}` (and sometimes
/// `{"msg": ...}` / `{"code": ...}` on older builds); we match both shapes.
fn classify_auth_error(status: u16, body: &Value) -> AuthError {
    let code = match body.get("error_code").or_else(|| body.get("code")) {
        Some(Value::String(s)) => s.to_lowercase(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    };
    let msg = body
        .get("error")
        .or_else(|| body.get("msg"))
        .or_else(|| body.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();
    let bad_credentials =
        code == "invalid_credentials" || code == "email_not_confirmed" || msg.contains("invalid login credentials") || msg.contains("email not confirmed");
    let refresh_dead = code.contains("refresh_token") || code.contains("refresh token");
    let email_exists = code == "email_exists"
        || msg.contains("already registered")
        || msg.contains("already exists");
    if bad_credentials {
        AuthError::BadCredentials
    } else if refresh_dead {
        AuthError::NoSession
    } else if email_exists {
        AuthError::EmailAlreadyExists
    } else {
        AuthError::Server {
            status,
            message: msg.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dasauto_core::entities::FuelType;
    use dasauto_core::store::MemoryStore;
    use dasauto_core::Utc;
    use dasauto_core::Uuid;

    fn vehicle() -> Vehicle {
        Vehicle {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            make: "VW".into(),
            model: "Golf".into(),
            version: Some("GTD".into()),
            year: 2019,
            plate: "FGB1E19".into(),
            renavam: Some("12345678901".into()),
            fuel_type: FuelType::Gasoline,
            state_uo: "SC".into(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn unconfigured_client_reports_unconfigured() {
        let c = SupabaseClient::new(None, None, Arc::new(MemoryStore::default()));
        assert!(!c.is_configured());
        assert_eq!(c.stored_session(), None);
    }

    #[test]
    fn is_configured_requires_both_parts() {
        let s = Arc::new(MemoryStore::default());
        assert!(!SupabaseClient::new(Some("https://x".into()), None, s.clone()).is_configured());
        assert!(!SupabaseClient::new(None, Some("key".into()), s.clone()).is_configured());
        assert!(SupabaseClient::new(Some("https://x".into()), Some("key".into()), s).is_configured());
    }

    #[test]
    fn session_persists_through_the_store() {
        let store: Arc<dyn LocalStore> = Arc::new(MemoryStore::default());
        let c = SupabaseClient::new(
            Some("https://x".into()),
            Some("key".into()),
            store.clone(),
        );
        let s = Session {
            access_token: "at".into(),
            refresh_token: "rt".into(),
            expires_at: i64::MAX,
            user_id: "u".into(),
            email: "a@b.c".into(),
        };
        c.set_session(Some(s.clone()));
        assert_eq!(c.stored_session(), Some(s.clone()));
        // A fresh client over the same store restores from persistence.
        let c2 = SupabaseClient::new(
            Some("https://x".into()),
            Some("key".into()),
            store,
        );
        assert_eq!(c2.stored_session(), Some(s));
    }

    #[test]
    fn corrupt_persisted_session_is_treated_as_missing() {
        let store: Arc<dyn LocalStore> = Arc::new(MemoryStore::default());
        store.set_raw(SESSION_KEY, "{ nope").unwrap();
        let c = SupabaseClient::new(
            Some("https://x".into()),
            Some("key".into()),
            store,
        );
        assert_eq!(c.stored_session(), None);
    }

    fn gotrue_session_json(expires_at: Option<i64>) -> Value {
        let mut v = json!({
            "access_token": "at",
            "token_type": "bearer",
            "refresh_token": "rt",
            "user": { "id": "11111111-1111-1111-1111-111111111111", "email": "a@b.c" },
        });
        if let Some(e) = expires_at {
            v["expires_at"] = json!(e);
        }
        v
    }

    #[test]
    fn parse_session_from_gotrue_shape() {
        let s = parse_session(&gotrue_session_json(Some(1_900_000_000)), "")
            .expect("should parse");
        assert_eq!(s.access_token, "at");
        assert_eq!(s.refresh_token, "rt");
        assert_eq!(s.expires_at, 1_900_000_000);
        assert_eq!(s.user_id, "11111111-1111-1111-1111-111111111111");
        assert_eq!(s.email, "a@b.c");
    }

    #[test]
    fn parse_session_falls_back_to_expires_in() {
        let body = json!({
            "access_token": "at",
            "refresh_token": "rt",
            "expires_in": 3600,
            "user": { "id": "u1" },
        });
        let s = parse_session(&body, "hint@x.com").expect("should parse");
        assert!(s.expires_at >= now_epoch_secs() + 3590);
        assert!(s.expires_at <= now_epoch_secs() + 3610);
        assert_eq!(s.email, "hint@x.com");
    }

    #[test]
    fn parse_session_rejects_user_only_body() {
        // Signup pending email confirmation: user object, no tokens.
        let body = json!({ "id": "u1", "email": "a@b.c" });
        assert!(parse_session(&body, "a@b.c").is_none());
    }

    #[test]
    fn parse_session_requires_user_id() {
        let body = json!({
            "access_token": "at",
            "refresh_token": "rt",
            "expires_at": 1,
            "user": { "email": "a@b.c" },
        });
        assert!(parse_session(&body, "").is_none());
    }

    #[test]
    fn classify_email_exists() {
        assert_eq!(
            classify_auth_error(400, &json!({ "error_code": "email_exists" })),
            AuthError::EmailAlreadyExists
        );
        assert_eq!(
            classify_auth_error(400, &json!({ "msg": "User already registered" })),
            AuthError::EmailAlreadyExists
        );
    }

    #[test]
    fn classify_bad_credentials() {
        assert_eq!(
            classify_auth_error(400, &json!({ "error_code": "invalid_credentials" })),
            AuthError::BadCredentials
        );
        assert_eq!(
            classify_auth_error(
                400,
                &json!({ "error": "Invalid login credentials" })
            ),
            AuthError::BadCredentials
        );
    }

    #[test]
    fn classify_dead_refresh_token_is_no_session() {
        assert_eq!(
            classify_auth_error(401, &json!({ "error_code": "refresh_token_expired" })),
            AuthError::NoSession
        );
        assert_eq!(
            classify_auth_error(401, &json!({ "error_code": "refresh_token_not_found" })),
            AuthError::NoSession
        );
    }

    #[test]
    fn classify_unknown_server_error() {
        let e = classify_auth_error(500, &json!({ "message": "boom" }));
        match e {
            AuthError::Server { status, message } => {
                assert_eq!(status, 500);
                assert_eq!(message, "boom");
            }
            other => panic!("expected Server, got {other:?}"),
        }
    }

    #[test]
    fn vehicle_payload_omits_user_and_timestamps() {
        let v = vehicle();
        let p = vehicle_payload(&v);
        assert_eq!(p["id"], json!(v.id));
        assert_eq!(p["make"], json!("VW"));
        assert_eq!(p["fuel_type"], json!("gasoline"));
        assert!(p.get("user_id").is_none(), "RLS sets user_id server-side");
        assert!(p.get("created_at").is_none());
        assert!(p.get("updated_at").is_none());
    }
}
