//! In-memory fakes for the service contracts (rules.md §2 — no network in
//! unit tests; §4 — core has no I/O dependencies, so the fakes carry no
//! state beyond `std::sync::Mutex`).
//!
//! Behind the `fakes` cargo feature so production builds never compile them.

use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

use uuid::Uuid;

use crate::api::{
    AuthError, AuthService, BoxFuture, RepoError, SignUpOutcome, VehicleService,
};
use crate::entities::Vehicle;
use crate::session::{Profile, Session};

/// A ready-made boxed future — no executor needed to run a fake.
fn ready<'a, T>(value: T) -> BoxFuture<'a, T> {
    Pin::Box::new(std::future::ready(value))
}

/// Deterministic `AuthService` for tests.
///
/// Unset fields fall back to `None`/empty (no session), so a test controls
/// exactly what each call returns.
#[derive(Clone, Default)]
pub struct FakeAuth {
    pub stored: Mutex<Option<Session>>,
    pub sign_up: Mutex<Option<Result<SignUpOutcome, AuthError>>>,
    pub sign_in: Mutex<Option<Result<Session, AuthError>>>,
    pub restore: Mutex<Option<Result<Option<Session>, AuthError>>>,
    pub sign_out_err: Mutex<Option<AuthError>>,
    pub pkce: Mutex<Option<Result<Session, AuthError>>>,
}

impl FakeAuth {
    fn default_session() -> Session {
        Session {
            access_token: "at".into(),
            refresh_token: "rt".into(),
            expires_at: i64::MAX,
            user_id: Uuid::new_v4().to_string(),
            email: "user@teste.com".into(),
        }
    }
}

impl AuthService for FakeAuth {
    fn sign_up_email<'a>(
        &'a self,
        _email: &'a str,
        _password: &'a str,
    ) -> BoxFuture<'a, Result<SignUpOutcome, AuthError>> {
        let r = self.sign_up.lock().unwrap().clone();
        match r {
            Some(outcome) => ready(outcome),
            None => {
                let s = Self::default_session();
                *self.stored.lock().unwrap() = Some(s.clone());
                ready(Ok(SignUpOutcome::Authenticated(s)))
            }
        }
    }

    fn sign_in_email<'a>(
        &'a self,
        _email: &'a str,
        _password: &'a str,
    ) -> BoxFuture<'a, Result<Session, AuthError>> {
        let r = self.sign_in.lock().unwrap().clone();
        match r {
            Some(result) => ready(result),
            None => {
                let s = Self::default_session();
                *self.stored.lock().unwrap() = Some(s.clone());
                ready(Ok(s))
            }
        }
    }

    fn restore_session(&self) -> BoxFuture<'_, Result<Option<Session>, AuthError>> {
        let explicit = self.restore.lock().unwrap().clone();
        match explicit {
            Some(r) => ready(r),
            None => ready(self.stored.lock().unwrap().clone().map(Ok)),
        }
    }

    fn sign_out(&self) -> BoxFuture<'_, Result<(), AuthError>> {
        let err = self.sign_out_err.lock().unwrap().clone();
        *self.stored.lock().unwrap() = None;
        ready(err.map(Err).unwrap_or(Ok(())))
    }

    fn exchange_pkce(&self, _code: &str, _verifier: &str) -> BoxFuture<'_, Result<Session, AuthError>> {
        let r = self.pkce.lock().unwrap().clone();
        match r {
            Some(result) => ready(result),
            None => {
                let s = Self::default_session();
                *self.stored.lock().unwrap() = Some(s.clone());
                ready(Ok(s))
            }
        }
    }

    fn stored_session(&self) -> Option<Session> {
        self.stored.lock().unwrap().clone()
    }

    fn current_profile(&self) -> Option<Profile> {
        self.stored_session().map(|s| s.profile())
    }
}

/// Deterministic `VehicleService` for tests.
///
/// `create` appends to `rows` (offline-first: local write then server);
/// `remove` filters by id. Error fields, when set, override the default
/// success behavior.
#[derive(Clone, Default)]
pub struct FakeVehicles {
    pub rows: Mutex<Vec<Vehicle>>,
    pub list_err: Mutex<Option<RepoError>>,
    pub create_err: Mutex<Option<RepoError>>,
    pub remove_err: Mutex<Option<RepoError>>,
    pub last_payload: Mutex<Option<Vehicle>>,
}

impl VehicleService for FakeVehicles {
    fn list(&self) -> BoxFuture<'_, Result<Vec<Vehicle>, RepoError>> {
        if let Some(e) = *self.list_err.lock().unwrap() {
            return ready(Err(e));
        }
        ready(Ok(self.rows.lock().unwrap().clone()))
    }

    fn create(&self, vehicle: Vehicle) -> BoxFuture<'_, Result<Vehicle, RepoError>> {
        if let Some(e) = *self.create_err.lock().unwrap() {
            return ready(Err(e));
        }
        *self.last_payload.lock().unwrap() = Some(vehicle.clone());
        self.rows.lock().unwrap().push(vehicle.clone());
        ready(Ok(vehicle))
    }

    fn remove(&self, id: Uuid) -> BoxFuture<'_, Result<(), RepoError>> {
        if let Some(e) = *self.remove_err.lock().unwrap() {
            return ready(Err(e));
        }
        self.rows.lock().unwrap().retain(|v| v.id != id);
        ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run<F, T>(fut: F) -> T
    where
        F: Future<Output = T> + 'static,
    {
        futures::executor::block_on(fut)
    }

    fn vehicle(id: Uuid) -> Vehicle {
        Vehicle {
            id,
            user_id: Uuid::new_v4(),
            make: "Fiat".into(),
            model: "Uno".into(),
            version: None,
            year: 2015,
            plate: "ABC1D23".into(),
            renavam: None,
            fuel_type: crate::entities::FuelType::Flex,
            state_uo: "SC".into(),
            created_at: crate::Utc::now(),
            updated_at: crate::Utc::now(),
        }
    }

    #[test]
    fn auth_defaults_and_overrides() {
        let f = FakeAuth::default();
        // unset → defaults
        let s = run(f.sign_in_email("a@b.c", "x")).unwrap();
        assert_eq!(f.current_profile().unwrap().email, "user@teste.com");
        // explicit override
        *f.sign_in.lock().unwrap() = Some(Err(AuthError::BadCredentials));
        assert_eq!(
            run(f.sign_in_email("a@b.c", "x")).unwrap_err(),
            AuthError::BadCredentials
        );
    }

    #[test]
    fn vehicles_crud_round_trip() {
        let f = FakeVehicles::default();
        let v = vehicle(Uuid::new_v4());
        run(f.create(v.clone())).unwrap();
        assert_eq!(run(f.list()).unwrap(), vec![v.clone()]);
        run(f.remove(v.id)).unwrap();
        assert!(run(f.list()).unwrap().is_empty());
    }

    #[test]
    fn repo_error_maps_auth_errors() {
        assert_eq!(
            RepoError::from(AuthError::BadCredentials),
            RepoError::NoSession
        );
        assert_eq!(RepoError::from(AuthError::Network), RepoError::Network);
    }
}
