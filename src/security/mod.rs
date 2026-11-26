//! Server-side security primitives. Cross-origin protection is on by default in [`App`](crate::App);
//! sessions, rate limits and CORS are opt-in. `SqliteStore` needs the `sqlite` feature.

mod cors;
mod csrf;
pub mod password;
mod rate_limit;
mod session;
#[cfg(feature = "sqlite")]
mod sqlite;

use std::{future::Future, pin::Pin};

pub use cors::Cors;
pub use csrf::{CrossOrigin, CrossOriginService};
pub use rate_limit::{ClientKey, RateLimit, RateLimitService};
pub use session::{Auth, MemoryStore, Record, SameSite, Session, SessionService, SessionStore, Sessions};
#[cfg(feature = "sqlite")]
pub use sqlite::SqliteStore;
#[cfg(feature = "sqlite")]
pub use rusqlite;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// 32 bytes from the OS generator, base64url without padding (43 chars).
pub(crate) fn random_token() -> String {
    use base64::Engine;
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).expect("OS random generator failed");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub(crate) fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;
    Sha256::digest(data).iter().fold(String::with_capacity(64), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}
