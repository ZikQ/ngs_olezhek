//! Argon2id password hashing with the OWASP baseline parameters (19 MiB, t=2, p=1).
//! A hash takes tens of milliseconds on purpose, so both functions run on the blocking pool.

use std::sync::OnceLock;

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};

/// A PHC string (`$argon2id$v=19$...`) with a random salt, ready to store.
pub async fn hash(password: impl Into<String>) -> String {
    let password = password.into();
    tokio::task::spawn_blocking(move || hash_blocking(&password)).await.expect("password hashing panicked")
}

/// `None` (no such user) still runs a full verification against a dummy hash, so the response
/// time does not reveal which logins exist.
pub async fn verify(password: impl Into<String>, hash: Option<&str>) -> bool {
    let password = password.into();
    let hash = hash.map(str::to_owned);
    tokio::task::spawn_blocking(move || match hash {
        Some(h) => verify_blocking(&password, &h),
        None => {
            verify_blocking(&password, dummy());
            false
        }
    })
    .await
    .unwrap_or(false)
}

fn hash_blocking(password: &str) -> String {
    let hash: PasswordHash = Argon2::default().hash_password(password.as_bytes()).expect("OS random generator failed");
    hash.to_string()
}

fn verify_blocking(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash).is_ok_and(|h| Argon2::default().verify_password(password.as_bytes(), &h).is_ok())
}

fn dummy() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| hash_blocking("ngs-olezhek-dummy-password"))
}
