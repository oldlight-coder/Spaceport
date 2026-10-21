//! Minimal client for Stremio's public account API (`api.strem.io`).
//!
//! Every call is a POST to `{base}/api/{method}` with a JSON body. Account calls carry a `type`
//! field; datastore calls name a `collection` instead.
//! Responses are wrapped as `{"result": ...}` on success or `{"error": {...}}` on failure.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;

use super::addons::AddonCollection;

pub const DEFAULT_BASE_URL: &str = "https://api.strem.io";

/// Default time limit for a single addon request.
pub const ADDON_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// The request never got a usable answer (offline, DNS, TLS, timeout, bad HTTP status).
    #[error("could not reach Stremio: {0}")]
    Transport(String),
    /// Stremio answered with an error object, e.g. wrong password or expired session.
    #[error("{message}")]
    Stremio { code: i64, message: String },
    /// An addon URL could not be loaded or did not return a manifest.
    #[error("{0}")]
    Addon(String),
    /// Stremio answered with something we could not parse.
    #[error("unexpected response from Stremio: {0}")]
    InvalidResponse(String),
}

/// The account fields Spaceport uses. Stremio returns more; the rest are ignored.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct User {
    #[serde(rename = "_id")]
    pub id: String,
    pub email: String,
}

/// Result of a successful login. Not `Debug` so the auth key cannot end up in logs.
#[derive(Deserialize)]
pub struct LoginResult {
    #[serde(rename = "authKey")]
    pub auth_key: String,
    pub user: User,
}

#[derive(Deserialize)]
struct ErrorBody {
    #[serde(default)]
    code: i64,
    #[serde(default)]
    message: String,
}

#[derive(Deserialize)]
struct Envelope<T> {
    result: Option<T>,
    error: Option<ErrorBody>,
}

/// Most redirects one request may follow.
const MAX_REDIRECTS: usize = 5;

/// Whether a request that started at `first` may follow a redirect to `next`. The same host is
/// fine; another host must be a public https one, so an addon cannot point requests at this
/// computer or the local network. Stremio's account API may only redirect within strem.io, as
/// a redirect re-sends the request body with the auth key.
fn redirect_allowed(first: &reqwest::Url, next: &reqwest::Url) -> bool {
    use std::net::IpAddr;
    let (Some(from), Some(to)) = (first.host_str(), next.host_str()) else {
        return false;
    };
    if from == "api.strem.io" {
        return next.scheme() == "https" && (to == "strem.io" || to.ends_with(".strem.io"));
    }
    if from.eq_ignore_ascii_case(to) && matches!(next.scheme(), "https" | "http") {
        return true;
    }
    let public = match next.host() {
        Some(url::Host::Domain(d)) => {
            let d = d.to_ascii_lowercase();
            d != "localhost" && !d.ends_with(".localhost") && !d.ends_with(".local")
        }
        Some(url::Host::Ipv4(ip)) => {
            let ip = IpAddr::V4(ip);
            !(ip.is_loopback() || ip.is_unspecified() || is_local(&ip))
        }
        Some(url::Host::Ipv6(ip)) => {
            let ip = IpAddr::V6(ip);
            !(ip.is_loopback() || ip.is_unspecified() || is_local(&ip))
        }
        None => false,
    };
    next.scheme() == "https" && public
}

/// Private, link-local and other addresses that only make sense on this network.
fn is_local(ip: &std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v4) => v4.is_private() || v4.is_link_local() || v4.is_broadcast(),
        std::net::IpAddr::V6(v6) => {
            let first = v6.segments()[0];
            (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80
        }
    }
}

/// A request failure in plain words. reqwest's own text ("error sending request") does not say
/// that the computer is offline; the URL is left out because addon URLs can embed tokens.
fn describe(error: reqwest::Error) -> String {
    if error.is_timeout() {
        "the request timed out".to_string()
    } else if error.is_connect() {
        "no connection (are you offline?)".to_string()
    } else if let Some(status) = error.status() {
        format!("HTTP {status}")
    } else {
        error.without_url().to_string()
    }
}

/// Turns a raw response body into the `result` value or an `ApiError`.
fn parse_envelope<T: DeserializeOwned>(body: &str) -> Result<T, ApiError> {
    let envelope: Envelope<T> =
        serde_json::from_str(body).map_err(|e| ApiError::InvalidResponse(e.to_string()))?;
    if let Some(error) = envelope.error {
        let message = if error.message.is_empty() {
            "Stremio returned an error".to_string()
        } else {
            error.message
        };
        return Err(ApiError::Stremio {
            code: error.code,
            message,
        });
    }
    envelope
        .result
        .ok_or_else(|| ApiError::InvalidResponse("missing result".to_string()))
}

#[derive(Clone)]
pub struct StremioApi {
    http: reqwest::Client,
    base_url: String,
}

impl StremioApi {
    pub fn new(base_url: impl Into<String>) -> Self {
        // Some services (behind Cloudflare) refuse requests that do not name their client.
        let http = reqwest::Client::builder()
            .user_agent(concat!("Spaceport/", env!("CARGO_PKG_VERSION")))
            // Addon URLs can hold debrid keys; a Referer would hand them to the next host.
            .referer(false)
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                let first = attempt.previous().first().cloned();
                let allowed = attempt.previous().len() <= MAX_REDIRECTS
                    && first.is_some_and(|first| redirect_allowed(&first, attempt.url()));
                if allowed {
                    attempt.follow()
                } else {
                    attempt.stop()
                }
            }))
            .timeout(Duration::from_secs(20))
            .build()
            .expect("failed to build HTTP client");
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    async fn call<T: DeserializeOwned>(&self, method: &str, body: Value) -> Result<T, ApiError> {
        let url = format!("{}/api/{}", self.base_url, method);
        let response = self
            .http
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ApiError::Transport(describe(e)))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| ApiError::Transport(describe(e)))?;
        // Stremio reports API errors in the body, sometimes with a non-2xx status, so try the
        // body first and only fall back to the status when it is not a valid envelope.
        match parse_envelope(&text) {
            Err(ApiError::InvalidResponse(_)) if !status.is_success() => {
                Err(ApiError::Transport(format!("HTTP {status}")))
            }
            other => other,
        }
    }

    pub async fn login(&self, email: &str, password: &str) -> Result<LoginResult, ApiError> {
        self.call(
            "login",
            json!({ "type": "Login", "email": email, "password": password, "facebook": false }),
        )
        .await
    }

    pub async fn logout(&self, auth_key: &str) -> Result<(), ApiError> {
        let _: Value = self
            .call("logout", json!({ "type": "Logout", "authKey": auth_key }))
            .await?;
        Ok(())
    }

    pub async fn get_user(&self, auth_key: &str) -> Result<User, ApiError> {
        self.call("getUser", json!({ "type": "GetUser", "authKey": auth_key }))
            .await
    }

    /// Installed addons in the user's order.
    pub async fn addon_collection(&self, auth_key: &str) -> Result<Vec<Value>, ApiError> {
        let collection: AddonCollection = self
            .call(
                "addonCollectionGet",
                json!({ "type": "AddonCollectionGet", "authKey": auth_key, "update": true }),
            )
            .await?;
        Ok(collection.addons)
    }

    /// Replaces the account's addon collection, which also sets the order.
    pub async fn set_addon_collection(
        &self,
        auth_key: &str,
        addons: &[Value],
    ) -> Result<(), ApiError> {
        let _: Value = self
            .call(
                "addonCollectionSet",
                json!({ "type": "AddonCollectionSet", "authKey": auth_key, "addons": addons }),
            )
            .await?;
        Ok(())
    }

    /// Downloads an addon manifest from the URL the user entered.
    pub async fn fetch_manifest(&self, url: &str) -> Result<Value, ApiError> {
        self.addon_json(url, ADDON_TIMEOUT).await
    }

    /// GETs a JSON resource from an installed addon with its own timeout, so one slow addon
    /// cannot hold up the others.
    pub async fn addon_json(&self, url: &str, timeout: Duration) -> Result<Value, ApiError> {
        let response = self
            .http
            .get(url)
            .timeout(timeout)
            .send()
            .await
            .and_then(|r| r.error_for_status())
            // Configured addon URLs can embed tokens, so keep the URL out of the message.
            .map_err(|e| ApiError::Addon(format!("could not load the addon: {}", describe(e))))?;
        response
            .json()
            .await
            .map_err(|_| ApiError::Addon("the addon did not return JSON".to_string()))
    }

    /// GETs JSON from one of the other allowed services (Kitsu, AniList, AniSkip). A 404 is
    /// returned as `Ok(None)`: these services answer that way when they know nothing.
    /// `accept` is the media type to ask for; Kitsu answers 406 unless it is JSON:API's.
    pub async fn service_get(
        &self,
        url: &str,
        accept: &str,
        timeout: Duration,
    ) -> Result<Option<Value>, ApiError> {
        let response = self
            .http
            .get(url)
            .header("Accept", accept)
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| ApiError::Transport(describe(e)))?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(ApiError::Transport(format!("HTTP {status}")));
        }
        response
            .json()
            .await
            .map(Some)
            .map_err(|e| ApiError::InvalidResponse(e.to_string()))
    }

    /// POSTs a JSON body to one of the other allowed services (AniList's GraphQL endpoint).
    pub async fn service_post(
        &self,
        url: &str,
        body: &Value,
        timeout: Duration,
    ) -> Result<Value, ApiError> {
        let response = self
            .http
            .post(url)
            .header("Accept", "application/json")
            .json(body)
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| ApiError::Transport(describe(e)))?;
        let status = response.status();
        if !status.is_success() {
            return Err(ApiError::Transport(format!("HTTP {status}")));
        }
        response
            .json()
            .await
            .map_err(|e| ApiError::InvalidResponse(e.to_string()))
    }

    /// Every entry of the library datastore, including removed and temporary ones, as stored.
    pub async fn library_items(&self, auth_key: &str) -> Result<Vec<Value>, ApiError> {
        self.call(
            "datastoreGet",
            json!({ "authKey": auth_key, "collection": "libraryItem", "ids": [], "all": true }),
        )
        .await
    }

    /// One library entry, if the title has one.
    pub async fn library_item(&self, auth_key: &str, id: &str) -> Result<Option<Value>, ApiError> {
        let items: Vec<Value> = self
            .call(
                "datastoreGet",
                json!({ "authKey": auth_key, "collection": "libraryItem", "ids": [id], "all": false }),
            )
            .await?;
        Ok(items
            .into_iter()
            .find(|i| i.get("_id").and_then(Value::as_str) == Some(id)))
    }

    /// Writes library entries back; each replaces the stored entry with the same `_id`.
    pub async fn put_library_items(&self, auth_key: &str, items: &[Value]) -> Result<(), ApiError> {
        let _: Value = self
            .call(
                "datastorePut",
                json!({ "authKey": auth_key, "collection": "libraryItem", "changes": items }),
            )
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_result() {
        let user: User =
            parse_envelope(r#"{"result":{"_id":"u1","email":"a@b.c","fullname":""}}"#).unwrap();
        assert_eq!(
            user,
            User {
                id: "u1".into(),
                email: "a@b.c".into()
            }
        );
    }

    #[test]
    fn parses_error() {
        let err = parse_envelope::<User>(r#"{"error":{"message":"Wrong password","code":3}}"#)
            .err()
            .unwrap();
        match err {
            ApiError::Stremio { code, message } => {
                assert_eq!(code, 3);
                assert_eq!(message, "Wrong password");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn error_without_message_gets_default_text() {
        let err = parse_envelope::<User>(r#"{"error":{}}"#).err().unwrap();
        assert_eq!(err.to_string(), "Stremio returned an error");
    }

    #[test]
    fn rejects_garbage_and_empty_envelope() {
        assert!(matches!(
            parse_envelope::<User>("<html>"),
            Err(ApiError::InvalidResponse(_))
        ));
        assert!(matches!(
            parse_envelope::<User>("{}"),
            Err(ApiError::InvalidResponse(_))
        ));
    }

    #[tokio::test]
    async fn names_a_refused_connection() {
        // Nothing listens on a port that was just freed, like a server that cannot be reached.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let api = StremioApi::new(format!("http://127.0.0.1:{port}"));
        let err = api.get_user("k").await.err().unwrap();
        assert_eq!(
            err.to_string(),
            "could not reach Stremio: no connection (are you offline?)"
        );
    }

    #[test]
    fn redirects_stay_public_and_account_calls_stay_on_strem_io() {
        let url = |s: &str| reqwest::Url::parse(s).unwrap();
        let addon = url("https://addon.example/key=abc/stream/movie/tt1.json");
        assert!(redirect_allowed(
            &addon,
            &url("https://addon.example/v2/stream.json")
        ));
        assert!(redirect_allowed(
            &addon,
            &url("https://cdn.example.net/stream.json")
        ));
        assert!(!redirect_allowed(
            &addon,
            &url("http://cdn.example.net/stream.json")
        ));
        for local in [
            "https://127.0.0.1:8080/stream/x",
            "https://localhost/x",
            "https://192.168.1.1/admin",
            "https://10.0.0.5/",
            "https://169.254.169.254/latest",
            "https://[::1]/x",
            "https://[fd00::1]/x",
            "file:///C:/Windows/win.ini",
        ] {
            assert!(!redirect_allowed(&addon, &url(local)), "{local}");
        }
        // A local addon the user installed may redirect within itself.
        let local_addon = url("http://127.0.0.1:7000/manifest.json");
        assert!(redirect_allowed(
            &local_addon,
            &url("http://127.0.0.1:7000/v2/manifest.json")
        ));

        let api = url("https://api.strem.io/api/getUser");
        assert!(redirect_allowed(
            &api,
            &url("https://www.strem.io/api/getUser")
        ));
        assert!(!redirect_allowed(
            &api,
            &url("https://evil.example/collect")
        ));
        assert!(!redirect_allowed(
            &api,
            &url("http://api.strem.io/api/getUser")
        ));
    }

    #[test]
    fn parses_login_result() {
        let r: LoginResult =
            parse_envelope(r#"{"result":{"authKey":"k","user":{"_id":"u1","email":"a@b.c"}}}"#)
                .unwrap();
        assert_eq!(r.auth_key, "k");
        assert_eq!(r.user.email, "a@b.c");
    }
}
