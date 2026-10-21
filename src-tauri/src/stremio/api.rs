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
        let http = reqwest::Client::builder()
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
            .map_err(|e| ApiError::Transport(e.without_url().to_string()))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| ApiError::Transport(e.without_url().to_string()))?;
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
            .map_err(|e| {
                ApiError::Addon(format!("could not load the addon: {}", e.without_url()))
            })?;
        response
            .json()
            .await
            .map_err(|_| ApiError::Addon("the addon did not return JSON".to_string()))
    }

    /// GETs JSON from one of the other allowed services (Kitsu, AniList, AniSkip). A 404 is
    /// returned as `Ok(None)`: these services answer that way when they know nothing.
    pub async fn service_get(
        &self,
        url: &str,
        timeout: Duration,
    ) -> Result<Option<Value>, ApiError> {
        let response = self
            .http
            .get(url)
            .header("Accept", "application/json")
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| ApiError::Transport(e.without_url().to_string()))?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let response = response
            .error_for_status()
            .map_err(|e| ApiError::Transport(e.without_url().to_string()))?;
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
            .and_then(|r| r.error_for_status())
            .map_err(|e| ApiError::Transport(e.without_url().to_string()))?;
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

    #[test]
    fn parses_login_result() {
        let r: LoginResult =
            parse_envelope(r#"{"result":{"authKey":"k","user":{"_id":"u1","email":"a@b.c"}}}"#)
                .unwrap();
        assert_eq!(r.auth_key, "k");
        assert_eq!(r.user.email, "a@b.c");
    }
}
