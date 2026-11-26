use std::time::Duration;

use axum::http::{HeaderName, HeaderValue, Method, header};
use tower_http::cors::{AllowOrigin, CorsLayer};

/// Lets pages on the listed origins call the API from the browser. `App::cors` also trusts
/// these origins in the cross-origin protection, otherwise their POSTs would be rejected.
#[derive(Clone, Debug)]
pub struct Cors {
    origins: Vec<HeaderValue>,
    credentials: bool,
    headers: Vec<HeaderName>,
    max_age: Duration,
}

impl Cors {
    /// Exact origins only (`https://admin.example.com`); there is no wildcard on purpose.
    /// Panics on a value that is not a valid header, like other startup configuration.
    pub fn origins<I, S>(origins: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let origins = origins
            .into_iter()
            .map(|o| {
                let o = o.as_ref().trim().trim_end_matches('/');
                assert!(o != "*" && o.contains("://"), "CORS origin must be scheme://host[:port], got {o:?}");
                HeaderValue::from_str(o).unwrap_or_else(|_| panic!("invalid CORS origin {o:?}"))
            })
            .collect();
        Self { origins, credentials: false, headers: vec![header::CONTENT_TYPE, header::AUTHORIZATION], max_age: Duration::from_secs(3600) }
    }

    /// Send cookies with cross-origin requests. Off by default.
    pub fn credentials(mut self, on: bool) -> Self {
        self.credentials = on;
        self
    }

    pub fn allow_header(mut self, name: HeaderName) -> Self {
        self.headers.push(name);
        self
    }

    pub fn max_age(mut self, age: Duration) -> Self {
        self.max_age = age;
        self
    }

    pub fn origin_list(&self) -> impl Iterator<Item = &str> {
        self.origins.iter().filter_map(|o| o.to_str().ok())
    }

    pub fn layer(&self) -> CorsLayer {
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(self.origins.clone()))
            .allow_methods([Method::GET, Method::HEAD, Method::POST, Method::PUT, Method::PATCH, Method::DELETE])
            .allow_headers(self.headers.clone())
            .allow_credentials(self.credentials)
            .max_age(self.max_age)
    }
}
