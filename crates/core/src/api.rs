//! Platform-agnostic service contracts (rules.md §4: core never depends on
//! net/UI).
//!
//! The app depends only on these traits; `dasauto-net` implements them over
//! Supabase REST (GoTrue for auth, PostgREST for data). No HTTP types leak
//! into this module — the contracts speak domain types only.
//!
//! Signatures are async (they describe I/O boundaries), but this crate
//! stays pure: the implementations decide where the I/O happens.

use std::future::Future;

use crate::entities::Vehicle;
use crate::session::{Profile, Session};

/// A boxed 'static future — lets the trait stay object-safe (`Arc<dyn …>`).
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn Future<Output = T> + Send + Sync + 'a>>;

/// Errors from the auth service. Message text is never embedded here: the UI
/// maps each variant to a pt-BR string (rules.md §6, D12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// Wrong credentials (GoTrue `invalid_credentials`).
    BadCredentials,
    /// The email is already registered (GoTrue `email_exists`).
    EmailAlreadyExists,
    /// No usable session (expired refresh token, logged out, …).
    NoSession,
    /// Network failure (offline, DNS, TLS).
    Network,
    /// The client was built without a backend URL/key (build misconfig).
    Unconfigured,
    /// Anything else the server rejected.
    Server {
        status: u16,
        message: String,
    },
}

/// Outcome of email sign-up: Supabase may auto-confirm (session returned) or
/// require email confirmation first (no session in the response).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignUpOutcome {
    /// A session was returned; the user is signed in.
    Authenticated(Session),
    /// A confirmation email was sent; the user must sign in after confirming.
    ConfirmationSent,
}

/// Authentication (GoTrue via `dasauto-net`).
///
/// `restore_session`/`stored_session` are the two session sources the app
/// checks at startup; the impl persists the session in `LocalStore`
/// (`dasauto:session`) and auto-refreshes tokens near expiry (D17).
pub trait AuthService: Send + Sync {
    /// Email/password sign-up.
    fn sign_up_email<'a>(&'a self, email: &'a str, password: &'a str)
    -> BoxFuture<'a, Result<SignUpOutcome, AuthError>>;

    /// Email/password sign-in.
    fn sign_in_email<'a>(&'a self, email: &'a str, password: &'a str)
    -> BoxFuture<'a, Result<Session, AuthError>>;

    /// Restore the persisted session (refreshing if near expiry). `Ok(None)`
    /// means "no stored session".
    fn restore_session(&self) -> BoxFuture<'_, Result<Option<Session>, AuthError>>;

    /// Clear the session locally (and best-effort revoke remotely).
    fn sign_out(&self) -> BoxFuture<'_, Result<(), AuthError>>;

    /// Exchange a PKCE `code` (Google OAuth callback) for a session.
    fn exchange_pkce(&self, code: &str, verifier: &str) -> BoxFuture<'_, Result<Session, AuthError>>;

    /// The currently stored session, if any (restored or in-memory).
    fn stored_session(&self) -> Option<Session>;

    /// Convenience wrapper over `stored_session`.
    fn current_profile(&self) -> Option<Profile>;
}

/// Errors from the vehicle service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoError {
    /// No usable session — the UI prompts a re-login (rules.md §7 offline).
    NoSession,
    Network,
    Unconfigured,
    Server { status: u16, message: String },
}

impl From<AuthError> for RepoError {
    fn from(e: AuthError) -> Self {
        match e {
            AuthError::Network => RepoError::Network,
            AuthError::Unconfigured => RepoError::Unconfigured,
            // A dead session surfaces as "log in again", not as a server error.
            AuthError::NoSession | AuthError::BadCredentials => RepoError::NoSession,
            AuthError::EmailAlreadyExists => RepoError::Server {
                status: 400,
                message: "email already exists".into(),
            },
            AuthError::Server { status, message } => RepoError::Server { status, message },
        }
    }
}

/// Vehicle CRUD (PostgREST via `dasauto-net`).
///
/// `user_id` scoping is enforced by Postgres RLS server-side; these methods
/// take no user argument and never filter client-side (rules.md §8).
pub trait VehicleService: Send + Sync {
    /// All vehicles for the session user.
    fn list(&self) -> BoxFuture<'_, Result<Vec<Vehicle>, RepoError>>;

    /// Create a vehicle. The client pre-generates `id` (offline-ready);
    /// `user_id` and timestamps are set server-side by trigger/RLS and
    /// echoed back in the returned row.
    fn create(&self, vehicle: Vehicle) -> BoxFuture<'_, Result<Vehicle, RepoError>>;

    /// Delete a vehicle by id.
    fn remove(&self, id: uuid::Uuid) -> BoxFuture<'_, Result<(), RepoError>>;
}
