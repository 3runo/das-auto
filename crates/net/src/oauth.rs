//! PKCE (RFC 7636) wiring for the Supabase Google OAuth flow (plan.md §2:
//! "Auth SDK: Supabase ... Google").
//!
//! The pure helpers here are host-testable; the actual redirect and
//! code-exchange happen in the app (wasm) because they need the browser
//! location. The flow is gated on project configuration (Google provider
//! credentials + site URL in the Supabase dashboard — owner action).

/// Base64url (no padding) of SHA-256(bytes) — the S256 challenge method.
pub fn s256_challenge(verifier: &str) -> String {
    use base64::Engine as _;
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize())
}

/// A fresh PKCE pair: `(verifier, challenge)`.
///
/// The verifier is 72 hex chars (two UUIDs) — GoTrue requires 43..=128
/// characters. The verifier must be kept in `localStorage` until the
/// callback exchanges it.
pub fn pkce_pair() -> (String, String) {
    let verifier = format!(
        "{}{}",
        dasauto_core::Uuid::new_v4(),
        dasauto_core::Uuid::new_v4()
    );
    let challenge = s256_challenge(&verifier);
    (verifier, challenge)
}

/// The GoTrue authorize URL the app redirects to for the Google flow.
///
/// `redirect_to` is where GoTrue returns after the provider auth (must match
/// the project's configured site URL for the loop to close).
pub fn authorize_url(base_url: &str, challenge: &str, redirect_to: &str) -> String {
    format!(
        "{}/auth/v1/authorize?provider=google&code_challenge={challenge}&code_challenge_method=S256&redirect_to={}",
        base_url.trim_end_matches('/'),
        redirect_to
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc7636_appendix_b_test_vector() {
        // RFC 7636 Appendix B — the canonical S256 vector.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            s256_challenge(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn verifier_length_is_in_gotrue_range() {
        let (v, c) = pkce_pair();
        assert!((43..=128).contains(&v.len()), "verifier len {}", v.len());
        assert!(!c.is_empty());
        // A different pair every time.
        let (v2, c2) = pkce_pair();
        assert_ne!(v, v2);
        assert_ne!(c, c2);
    }

    #[test]
    fn challenge_is_deterministic() {
        let (v, c) = pkce_pair();
        assert_eq!(c, s256_challenge(&v));
    }

    #[test]
    fn authorize_url_shape() {
        let url = authorize_url(
            "https://x.supabase.co",
            "CH",
            "https://app.example/das-auto",
        );
        assert_eq!(
            url,
            "https://x.supabase.co/auth/v1/authorize?provider=google\
             &code_challenge=CH&code_challenge_method=S256\
             &redirect_to=https://app.example/das-auto"
        );
    }
}
