//! Stremio sign-in state. The auth key lives only in the secret store and in memory.

use serde::Serialize;

use crate::secrets::{SecretError, SecretStore};
use crate::stremio::api::{ApiError, StremioApi, User};

pub const AUTH_KEY_NAME: &str = "stremio-auth-key";

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error(transparent)]
    Secret(#[from] SecretError),
    #[error("not signed in")]
    NotSignedIn,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum Session {
    LoggedOut,
    LoggedIn { user: User },
}

pub async fn login(
    api: &StremioApi,
    store: &dyn SecretStore,
    email: &str,
    password: &str,
) -> Result<Session, AuthError> {
    let result = api.login(email.trim(), password).await?;
    store.set(AUTH_KEY_NAME, &result.auth_key)?;
    Ok(Session::LoggedIn { user: result.user })
}

/// Restores the session saved by a previous run. A key that Stremio no longer accepts is
/// removed; a network failure is returned as an error so the saved key is kept.
pub async fn restore(api: &StremioApi, store: &dyn SecretStore) -> Result<Session, AuthError> {
    let Some(auth_key) = store.get(AUTH_KEY_NAME)? else {
        return Ok(Session::LoggedOut);
    };
    match api.get_user(&auth_key).await {
        Ok(user) => Ok(Session::LoggedIn { user }),
        Err(ApiError::Stremio { .. }) => {
            store.delete(AUTH_KEY_NAME)?;
            Ok(Session::LoggedOut)
        }
        Err(e) => Err(e.into()),
    }
}

/// The saved auth key, for calls that need a signed-in user.
pub fn auth_key(store: &dyn SecretStore) -> Result<String, AuthError> {
    store.get(AUTH_KEY_NAME)?.ok_or(AuthError::NotSignedIn)
}

/// Ends the session on Stremio when possible and always forgets the local key.
pub async fn logout(api: &StremioApi, store: &dyn SecretStore) -> Result<Session, AuthError> {
    if let Some(auth_key) = store.get(AUTH_KEY_NAME)? {
        // Best effort: being offline must not keep the user signed in locally.
        let _ = api.logout(&auth_key).await;
    }
    store.delete(AUTH_KEY_NAME)?;
    Ok(Session::LoggedOut)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secrets::testing::MemoryStore;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Serves one canned JSON body per incoming request and returns the base URL.
    async fn mock_stremio(responses: Vec<&'static str>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            for body in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buf = vec![0u8; 8192];
                let _ = socket.read(&mut buf).await.unwrap();
                let reply = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                socket.write_all(reply.as_bytes()).await.unwrap();
            }
        });
        format!("http://{addr}")
    }

    fn user() -> User {
        User {
            id: "u1".into(),
            email: "a@b.c".into(),
        }
    }

    #[tokio::test]
    async fn login_saves_key() {
        let api = StremioApi::new(
            mock_stremio(vec![
                r#"{"result":{"authKey":"secret","user":{"_id":"u1","email":"a@b.c"}}}"#,
            ])
            .await,
        );
        let store = MemoryStore::default();
        let session = login(&api, &store, " a@b.c ", "pw").await.unwrap();
        assert_eq!(session, Session::LoggedIn { user: user() });
        assert_eq!(store.get(AUTH_KEY_NAME).unwrap().as_deref(), Some("secret"));
    }

    #[tokio::test]
    async fn failed_login_saves_nothing() {
        let api = StremioApi::new(
            mock_stremio(vec![r#"{"error":{"code":3,"message":"Wrong password"}}"#]).await,
        );
        let store = MemoryStore::default();
        let err = login(&api, &store, "a@b.c", "bad").await.err().unwrap();
        assert_eq!(err.to_string(), "Wrong password");
        assert_eq!(store.get(AUTH_KEY_NAME).unwrap(), None);
    }

    #[tokio::test]
    async fn restore_without_key_is_logged_out() {
        let api = StremioApi::new("http://127.0.0.1:9");
        let store = MemoryStore::default();
        assert_eq!(restore(&api, &store).await.unwrap(), Session::LoggedOut);
    }

    #[tokio::test]
    async fn restore_with_valid_key() {
        let api =
            StremioApi::new(mock_stremio(vec![r#"{"result":{"_id":"u1","email":"a@b.c"}}"#]).await);
        let store = MemoryStore::default();
        store.set(AUTH_KEY_NAME, "secret").unwrap();
        assert_eq!(
            restore(&api, &store).await.unwrap(),
            Session::LoggedIn { user: user() }
        );
    }

    #[tokio::test]
    async fn restore_with_rejected_key_clears_it() {
        let api = StremioApi::new(
            mock_stremio(vec![
                r#"{"error":{"code":1,"message":"Session does not exist"}}"#,
            ])
            .await,
        );
        let store = MemoryStore::default();
        store.set(AUTH_KEY_NAME, "old").unwrap();
        assert_eq!(restore(&api, &store).await.unwrap(), Session::LoggedOut);
        assert_eq!(store.get(AUTH_KEY_NAME).unwrap(), None);
    }

    #[tokio::test]
    async fn restore_offline_keeps_key() {
        // Port 9 (discard) is closed on loopback, so the request fails to connect.
        let api = StremioApi::new("http://127.0.0.1:9");
        let store = MemoryStore::default();
        store.set(AUTH_KEY_NAME, "secret").unwrap();
        assert!(matches!(
            restore(&api, &store).await,
            Err(AuthError::Api(ApiError::Transport(_)))
        ));
        assert_eq!(store.get(AUTH_KEY_NAME).unwrap().as_deref(), Some("secret"));
    }

    #[test]
    fn auth_key_requires_sign_in() {
        let store = MemoryStore::default();
        assert!(matches!(auth_key(&store), Err(AuthError::NotSignedIn)));
        store.set(AUTH_KEY_NAME, "secret").unwrap();
        assert_eq!(auth_key(&store).unwrap(), "secret");
    }

    #[tokio::test]
    async fn logout_clears_key_even_offline() {
        let api = StremioApi::new("http://127.0.0.1:9");
        let store = MemoryStore::default();
        store.set(AUTH_KEY_NAME, "secret").unwrap();
        assert_eq!(logout(&api, &store).await.unwrap(), Session::LoggedOut);
        assert_eq!(store.get(AUTH_KEY_NAME).unwrap(), None);
    }
}
