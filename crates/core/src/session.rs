//! Auth session state (plan.md §5.1, D17).
//!
//! The session shape mirrors GoTrue (Supabase Auth) so it can be persisted as
//! JSON in `LocalStore` and restored on startup. The publishable API key is
//! **never** stored here — only the per-user tokens (rules.md §8).

use serde::{Deserialize, Serialize};

/// Epoch seconds of "now".
///
/// The app runs single-threaded per process (wasm or main thread), so a
/// plain clock read is all token-expiry checks need.
pub fn now_epoch_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A live GoTrue session: the tokens plus the minimum user data the UI needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    /// Epoch seconds at which the access token expires.
    pub expires_at: i64,
    /// `auth.users.id` (Postgres UUID, string form).
    pub user_id: String,
    pub email: String,
}

impl Session {
    /// True when the access token expires within `leeway_secs` (or already).
    pub fn needs_refresh(&self, now: i64, leeway_secs: i64) -> bool {
        now >= self.expires_at - leeway_secs
    }

    /// The UI-facing profile derived from this session.
    pub fn profile(&self) -> Profile {
        Profile {
            id: self.user_id.clone(),
            email: self.email.clone(),
        }
    }
}

/// Minimum user data the UI displays (D28: no user profile editing in v1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub email: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(expires_at: i64) -> Session {
        Session {
            access_token: "at".into(),
            refresh_token: "rt".into(),
            expires_at,
            user_id: "u1".into(),
            email: "a@b.co".into(),
        }
    }

    #[test]
    fn serde_round_trip() {
        let s = session(1_700_000_000);
        let json = serde_json::to_string(&s).unwrap();
        let back: Session = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn refresh_boundaries() {
        // expires in 100s, leeway 60s → refresh now
        assert!(session(now_epoch_secs() + 100).needs_refresh(now_epoch_secs(), 60));
        // expires in 100s, leeway 50s → still fresh
        assert!(!session(now_epoch_secs() + 100).needs_refresh(now_epoch_secs(), 50));
        // already expired → refresh
        assert!(session(now_epoch_secs() - 5).needs_refresh(now_epoch_secs(), 0));
    }

    #[test]
    fn profile_carries_id_and_email() {
        let p = session(1).profile();
        assert_eq!(p.id, "u1");
        assert_eq!(p.email, "a@b.co");
    }
}
