//! Microsoft OAuth 2.0 authorization-code flow with a local loopback callback,
//! refresh-token renewal, and the Xbox Live/XSTS/Minecraft sign-in chain.

use std::collections::HashMap;
use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{Instant, timeout};
use url::Url;
use uuid::Uuid;

use crate::credentials::{AccountRecord, CredentialStore};

/// Public client ID registered for UMC's local-callback OAuth flow.
pub const MICROSOFT_CLIENT_ID: &str = "16e42c20-a7b7-45d3-a09e-0b86ff14d2e5";
/// Redirect URI registered in the Azure application. Native desktop apps use
/// the loopback root; the actual runtime URI adds an ephemeral port.
pub const DEFAULT_REDIRECT_URI: &str = "http://localhost";
/// Scopes required by the Minecraft/Xbox Live sign-in chain.
pub const DEFAULT_SCOPE: &str = "XboxLive.signin offline_access";

// XboxLive.signin is a consumer-account scope. The common tenant rejects it
// with invalid_scope for this application; consumers is the verified endpoint.
const DEFAULT_TENANT: &str = "consumers";
const XBL_USER_AUTH_URL: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS_AUTH_URL: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const XSTS_RELYING_PARTY: &str = "rp://api.minecraftservices.com/";
const MINECRAFT_LOGIN_URL: &str =
    "https://api.minecraftservices.com/authentication/login_with_xbox";
const MINECRAFT_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const AUTHORIZE_PATH: &str = "/oauth2/v2.0/authorize";
const TOKEN_PATH: &str = "/oauth2/v2.0/token";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const CALLBACK_REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_CALLBACK_REQUEST: usize = 16 * 1024;

/// Configuration for the Microsoft OAuth authorization-code flow.
#[derive(Debug, Clone)]
pub struct MicrosoftOAuthConfig {
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub tenant: String,
}

impl Default for MicrosoftOAuthConfig {
    fn default() -> Self {
        Self {
            client_id: MICROSOFT_CLIENT_ID.into(),
            redirect_uri: std::env::var("UMC_MS_REDIRECT_URI")
                .unwrap_or_else(|_| DEFAULT_REDIRECT_URI.into()),
            scope: std::env::var("UMC_MS_SCOPE").unwrap_or_else(|_| DEFAULT_SCOPE.into()),
            tenant: std::env::var("UMC_MS_TENANT").unwrap_or_else(|_| DEFAULT_TENANT.into()),
        }
    }
}

impl MicrosoftOAuthConfig {
    fn validate(&self) -> Result<(), String> {
        if self.client_id.trim().is_empty() {
            return Err("Microsoft client ID is empty".into());
        }
        if self.tenant.is_empty()
            || !self
                .tenant
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
        {
            return Err("Invalid Microsoft tenant".into());
        }
        if self.scope.trim().is_empty() {
            return Err("Microsoft OAuth scope is empty".into());
        }
        Ok(())
    }

    fn authorize_url(&self) -> Result<Url, String> {
        self.validate()?;
        let url = Url::parse(&format!(
            "https://login.microsoftonline.com/{}{}",
            self.tenant, AUTHORIZE_PATH
        ))
        .map_err(|e| format!("Invalid Microsoft authorization endpoint: {e}"))?;
        Ok(url)
    }

    fn token_url(&self) -> Result<Url, String> {
        self.validate()?;
        Url::parse(&format!(
            "https://login.microsoftonline.com/{}{}",
            self.tenant, TOKEN_PATH
        ))
        .map_err(|e| format!("Invalid Microsoft token endpoint: {e}"))
    }
}

/// Microsoft OAuth tokens returned by the token endpoint.
///
/// Tokens are intentionally not `Serialize` and their `Debug` output is
/// redacted so they cannot leak into logs or RPC responses by accident.
#[derive(Clone)]
pub struct MicrosoftOAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<u64>,
    pub scope: Option<String>,
    pub token_type: Option<String>,
    pub id_token: Option<String>,
}

impl fmt::Debug for MicrosoftOAuthTokens {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MicrosoftOAuthTokens")
            .field("access_token", &"<redacted>")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .field("expires_in", &self.expires_in)
            .field("scope", &self.scope)
            .field("token_type", &self.token_type)
            .field("id_token", &self.id_token.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl MicrosoftOAuthTokens {
    pub fn has_refresh_token(&self) -> bool {
        self.refresh_token.is_some()
    }
}

/// The Minecraft profile returned by `api.minecraftservices.com`.
#[derive(Debug, Clone, Deserialize)]
pub struct MinecraftProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub skins: serde_json::Value,
    #[serde(default)]
    pub capes: serde_json::Value,
}

/// A completed Microsoft/Xbox/Minecraft login.
///
/// Access and refresh tokens are intentionally not `Serialize`, and `Debug`
/// redacts them so a session cannot leak into logs or RPC output by accident.
pub struct MinecraftSession {
    pub microsoft_access_token: String,
    pub microsoft_refresh_token: Option<String>,
    pub microsoft_expires_in: Option<u64>,
    pub minecraft_access_token: String,
    pub minecraft_expires_in: Option<u64>,
    pub xuid: Option<String>,
    pub profile: MinecraftProfile,
}

impl fmt::Debug for MinecraftSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MinecraftSession")
            .field("microsoft_access_token", &"<redacted>")
            .field(
                "microsoft_refresh_token",
                &self.microsoft_refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .field("microsoft_expires_in", &self.microsoft_expires_in)
            .field("minecraft_access_token", &"<redacted>")
            .field("minecraft_expires_in", &self.minecraft_expires_in)
            .field("profile", &self.profile)
            .finish()
    }
}

struct XboxLiveAuth {
    token: String,
    user_hash: String,
    xuid: Option<String>,
}

#[derive(Deserialize)]
struct XboxTokenResponse {
    #[serde(rename = "Token")]
    token: String,
    #[serde(rename = "DisplayClaims")]
    display_claims: XboxDisplayClaims,
}

#[derive(Deserialize)]
struct XboxDisplayClaims {
    xui: Vec<XboxUserClaim>,
}

#[derive(Deserialize)]
struct XboxUserClaim {
    uhs: String,
    #[serde(default)]
    xid: Option<String>,
}

#[derive(Deserialize)]
struct XboxErrorResponse {
    #[serde(rename = "XErr", default)]
    xerr: Option<i64>,
    #[serde(rename = "Message", default)]
    message: Option<String>,
    #[serde(rename = "Redirect", default)]
    redirect: Option<String>,
}

#[derive(Deserialize)]
struct MinecraftLoginResponse {
    access_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
}

#[derive(Deserialize)]
struct MinecraftErrorResponse {
    #[serde(default)]
    error: Option<String>,
    #[serde(rename = "errorMessage", default)]
    error_message: Option<String>,
}

struct Pkce {
    state: String,
    verifier: String,
    challenge: String,
}

impl Pkce {
    fn generate() -> Self {
        let mut random = [0u8; 32];
        random[..16].copy_from_slice(Uuid::new_v4().as_bytes());
        random[16..].copy_from_slice(Uuid::new_v4().as_bytes());
        let verifier = URL_SAFE_NO_PAD.encode(random);
        let challenge = code_challenge(&verifier);
        Self {
            state: Uuid::new_v4().simple().to_string(),
            verifier,
            challenge,
        }
    }
}

fn code_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

fn authorization_url(config: &MicrosoftOAuthConfig, pkce: &Pkce) -> Result<String, String> {
    let mut url = config.authorize_url()?;
    url.query_pairs_mut()
        .append_pair("client_id", &config.client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", &config.redirect_uri)
        .append_pair("response_mode", "query")
        .append_pair("scope", &config.scope)
        .append_pair("state", &pkce.state)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("prompt", "select_account");
    // Microsoft's consumer login endpoint is more reliable with `%20` between
    // scopes than with the form-style `+` separator.
    let query = url.query().unwrap_or_default().replace('+', "%20");
    url.set_query(Some(&query));
    Ok(url.to_string())
}

/// A prepared Microsoft OAuth login. The local callback server is bound before
/// the browser is opened so the redirect cannot race the listener.
pub struct LoginFlow {
    config: MicrosoftOAuthConfig,
    pkce: Pkce,
    listeners: CallbackListeners,
    redirect_path: String,
    auth_url: String,
    browser_error: Option<String>,
}

impl LoginFlow {
    /// Starts a login with the default UMC client and redirect URI.
    pub async fn start(open_browser: bool) -> Result<Self, String> {
        Self::start_with_config(MicrosoftOAuthConfig::default(), open_browser).await
    }

    pub async fn start_with_config(
        config: MicrosoftOAuthConfig,
        should_open_browser: bool,
    ) -> Result<Self, String> {
        let mut config = config;
        config.validate()?;
        let (redirect_path, actual_redirect_uri, listeners) =
            CallbackListeners::bind(&config.redirect_uri).await?;
        config.redirect_uri = actual_redirect_uri;
        let pkce = Pkce::generate();
        let auth_url = authorization_url(&config, &pkce)?;
        let browser_error = if should_open_browser {
            open_browser(&auth_url).err()
        } else {
            None
        };
        Ok(Self {
            config,
            pkce,
            listeners,
            redirect_path,
            auth_url,
            browser_error,
        })
    }

    pub fn auth_url(&self) -> &str {
        &self.auth_url
    }

    /// The effective redirect URI, including the ephemeral loopback port.
    pub fn redirect_uri(&self) -> &str {
        &self.config.redirect_uri
    }

    pub fn browser_error(&self) -> Option<&str> {
        self.browser_error.as_deref()
    }

    /// Waits for the loopback callback and exchanges the authorization code.
    pub async fn wait(self) -> Result<MicrosoftOAuthTokens, String> {
        let code = self.wait_for_callback().await?;
        exchange_code(&self.config, &code, &self.pkce.verifier).await
    }

    async fn wait_for_callback(&self) -> Result<String, String> {
        let deadline = Instant::now() + CALLBACK_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err("Timed out waiting for the Microsoft sign-in callback".into());
            }

            let mut stream =
                timeout(remaining, self.listeners.accept())
                    .await
                    .map_err(|_| {
                        "Timed out waiting for the Microsoft sign-in callback".to_string()
                    })??;
            let request = read_http_request(&mut stream).await?;

            match parse_callback_request(&request, &self.pkce.state, &self.redirect_path) {
                Ok(Some(code)) => {
                    respond(
                        &mut stream,
                        200,
                        "OK",
                        "Microsoft sign-in complete",
                        "You can close this window and return to UMC.",
                    )
                    .await?;
                    return Ok(code);
                }
                Ok(None) => {
                    let _ = respond(
                        &mut stream,
                        404,
                        "Not Found",
                        "Not found",
                        "This local endpoint only handles the UMC Microsoft sign-in callback.",
                    )
                    .await;
                }
                Err(error) => {
                    let _ = respond(
                        &mut stream,
                        400,
                        "Bad Request",
                        "Microsoft sign-in failed",
                        "Return to the terminal for details.",
                    )
                    .await;
                    return Err(error);
                }
            }
        }
    }
}

struct CallbackListeners {
    v4: Option<TcpListener>,
    v6: Option<TcpListener>,
}

impl CallbackListeners {
    async fn bind(redirect_uri: &str) -> Result<(String, String, Self), String> {
        let (host, requested_port, path) = redirect_settings(redirect_uri)?;
        let (port, v4, v6) = match requested_port {
            Some(port) => {
                let v4 = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
                    .await
                    .ok();
                let v6 = TcpListener::bind(SocketAddr::from((Ipv6Addr::LOCALHOST, port)))
                    .await
                    .ok();
                if v4.is_none() && v6.is_none() {
                    return Err(format!(
                        "Could not bind the Microsoft callback server on loopback port {port}. \
                         Close the process using that port and make sure the redirect URI matches."
                    ));
                }
                (port, v4, v6)
            }
            None => bind_ephemeral_port().await?,
        };

        let actual_redirect_uri = if requested_port.is_some() {
            redirect_uri.to_owned()
        } else {
            let authority = if host.contains(':') {
                format!("[{host}]")
            } else {
                host
            };
            let suffix = if path == "/" {
                String::new()
            } else {
                path.clone()
            };
            format!("http://{authority}:{port}{suffix}")
        };

        Ok((path, actual_redirect_uri, Self { v4, v6 }))
    }

    async fn accept(&self) -> Result<TcpStream, String> {
        match (self.v4.as_ref(), self.v6.as_ref()) {
            (Some(v4), Some(v6)) => tokio::select! {
                result = v4.accept() => result.map(|(stream, _)| stream).map_err(|e| e.to_string()),
                result = v6.accept() => result.map(|(stream, _)| stream).map_err(|e| e.to_string()),
            },
            (Some(v4), None) => v4
                .accept()
                .await
                .map(|(stream, _)| stream)
                .map_err(|e| e.to_string()),
            (None, Some(v6)) => v6
                .accept()
                .await
                .map(|(stream, _)| stream)
                .map_err(|e| e.to_string()),
            (None, None) => Err("Microsoft callback server is not listening".into()),
        }
    }
}

async fn bind_ephemeral_port() -> Result<(u16, Option<TcpListener>, Option<TcpListener>), String> {
    if let Ok(v4) = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await {
        let port = v4
            .local_addr()
            .map_err(|e| format!("Could not read the Microsoft callback port: {e}"))?
            .port();
        let v6 = TcpListener::bind(SocketAddr::from((Ipv6Addr::LOCALHOST, port)))
            .await
            .ok();
        return Ok((port, Some(v4), v6));
    }

    let v6 = TcpListener::bind(SocketAddr::from((Ipv6Addr::LOCALHOST, 0)))
        .await
        .map_err(|e| format!("Could not bind the Microsoft callback server: {e}"))?;
    let port = v6
        .local_addr()
        .map_err(|e| format!("Could not read the Microsoft callback port: {e}"))?
        .port();
    Ok((port, None, Some(v6)))
}

fn redirect_settings(redirect_uri: &str) -> Result<(String, Option<u16>, String), String> {
    let url =
        Url::parse(redirect_uri).map_err(|e| format!("Invalid Microsoft redirect URI: {e}"))?;
    if url.scheme() != "http" {
        return Err("Microsoft redirect URI must use http on loopback".into());
    }
    let host = url
        .host_str()
        .ok_or("Microsoft redirect URI has no host")?
        .to_owned();
    if !matches!(host.as_str(), "localhost" | "127.0.0.1" | "::1") {
        return Err("Microsoft redirect URI must point to localhost or a loopback address".into());
    }
    Ok((host, url.port(), url.path().to_owned()))
}

async fn read_http_request(stream: &mut TcpStream) -> Result<String, String> {
    let mut buffer = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    loop {
        let read = timeout(CALLBACK_REQUEST_TIMEOUT, stream.read(&mut chunk))
            .await
            .map_err(|_| "Timed out reading the Microsoft callback request".to_string())?
            .map_err(|e| format!("Could not read the Microsoft callback request: {e}"))?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if buffer.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if buffer.len() > MAX_CALLBACK_REQUEST {
            return Err("Microsoft callback request is too large".into());
        }
    }
    String::from_utf8(buffer)
        .map_err(|_| "Microsoft callback request is not valid UTF-8".to_string())
}

fn parse_callback_request(
    request: &str,
    expected_state: &str,
    expected_path: &str,
) -> Result<Option<String>, String> {
    let request_line = request
        .lines()
        .next()
        .ok_or("Empty Microsoft callback request")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().ok_or("Malformed Microsoft callback request")?;
    let target = parts.next().ok_or("Malformed Microsoft callback request")?;
    if method != "GET" {
        return Err("Microsoft callback endpoint only accepts GET".into());
    }

    let url = Url::parse(&format!("http://localhost{target}"))
        .map_err(|e| format!("Malformed Microsoft callback URL: {e}"))?;
    if url.path() != expected_path {
        return Ok(None);
    }

    let params: HashMap<String, String> = url.query_pairs().into_owned().collect();
    if let Some(error) = params.get("error") {
        let description = params
            .get("error_description")
            .filter(|value| !value.is_empty());
        return Err(match description {
            Some(description) => format!("Microsoft sign-in failed: {description} ({error})"),
            None => format!("Microsoft sign-in failed: {error}"),
        });
    }

    match params.get("state") {
        Some(state) if state == expected_state => {}
        _ => return Err(
            "Microsoft sign-in state mismatch; the callback was not initiated by this login flow"
                .into(),
        ),
    }

    params
        .get("code")
        .cloned()
        .map(Some)
        .ok_or_else(|| "Microsoft callback did not include an authorization code".into())
}

async fn respond(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    title: &str,
    message: &str,
) -> Result<(), String> {
    let body = format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
body {{ margin: 0; min-height: 100vh; display: grid; place-items: center; background: #0b0d12; color: #f5f7fb; font: 16px/1.6 system-ui, sans-serif; }}
main {{ max-width: 520px; padding: 40px; text-align: center; }}
h1 {{ font-size: 24px; margin: 0 0 12px; }}
p {{ margin: 0; color: #aab2c5; }}
</style>
</head>
<body>
<main>
<h1>{title}</h1>
<p>{message}</p>
</main>
</body>
</html>"#
    );
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(response.as_bytes())
        .await
        .map_err(|e| format!("Could not write the Microsoft callback response: {e}"))?;
    stream
        .flush()
        .await
        .map_err(|e| format!("Could not flush the Microsoft callback response: {e}"))?;
    let _ = stream.shutdown().await;
    Ok(())
}

async fn exchange_code(
    config: &MicrosoftOAuthConfig,
    code: &str,
    verifier: &str,
) -> Result<MicrosoftOAuthTokens, String> {
    let client = crate::engine::http_client()?;
    let response = client
        .post(config.token_url()?)
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", config.redirect_uri.as_str()),
            ("code_verifier", verifier),
            ("scope", config.scope.as_str()),
        ])
        .send()
        .await
        .map_err(|e| format!("Microsoft token request failed: {e}"))?;

    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Could not read the Microsoft token response: {e}"))?;

    if !status.is_success() {
        let message = serde_json::from_slice::<OAuthError>(&bytes)
            .ok()
            .and_then(|error| {
                error
                    .error_description
                    .filter(|description| !description.is_empty())
                    .or(Some(error.error))
            })
            .unwrap_or_else(|| status.to_string());
        return Err(format!("Microsoft token exchange failed: {message}"));
    }

    let response: TokenResponse = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Invalid Microsoft token response: {e}"))?;
    if response.access_token.is_empty() {
        return Err("Microsoft token response did not include an access token".into());
    }
    Ok(MicrosoftOAuthTokens {
        access_token: response.access_token,
        refresh_token: response.refresh_token,
        expires_in: response.expires_in,
        scope: response.scope,
        token_type: response.token_type,
        id_token: response.id_token,
    })
}

/// A refresh-token request failed. `invalid_grant` means the stored credential
/// is no longer usable and the user must sign in again.
#[derive(Debug, Clone)]
pub struct RefreshError {
    pub invalid_grant: bool,
    pub message: String,
}

impl RefreshError {
    fn other(message: impl Into<String>) -> Self {
        Self {
            invalid_grant: false,
            message: message.into(),
        }
    }
}

impl fmt::Display for RefreshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RefreshError {}

/// Exchanges a Microsoft refresh token for a fresh access token.
pub async fn refresh_microsoft_tokens(
    config: &MicrosoftOAuthConfig,
    refresh_token: &str,
) -> Result<MicrosoftOAuthTokens, RefreshError> {
    config.validate().map_err(RefreshError::other)?;
    let client = crate::engine::http_client().map_err(RefreshError::other)?;
    let response = client
        .post(config.token_url().map_err(RefreshError::other)?)
        .form(&[
            ("client_id", config.client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("scope", config.scope.as_str()),
        ])
        .send()
        .await
        .map_err(|error| {
            RefreshError::other(format!("Microsoft refresh request failed: {error}"))
        })?;

    let status = response.status();
    let bytes = response.bytes().await.map_err(|error| {
        RefreshError::other(format!(
            "Could not read the Microsoft refresh response: {error}"
        ))
    })?;

    if !status.is_success() {
        let oauth_error = serde_json::from_slice::<OAuthError>(&bytes).ok();
        let invalid_grant = oauth_error.as_ref().is_some_and(|error| {
            error.error == "invalid_grant"
                || error.error == "interaction_required"
                || error
                    .error_description
                    .as_deref()
                    .is_some_and(|description| {
                        description.contains("AADSTS70008") || description.contains("AADSTS50173")
                    })
        });
        let message = oauth_error
            .and_then(|error| {
                error
                    .error_description
                    .filter(|description| !description.is_empty())
                    .or(Some(error.error))
            })
            .unwrap_or_else(|| status.to_string());
        return Err(RefreshError {
            invalid_grant,
            message: format!("Microsoft sign-in has expired: {message}"),
        });
    }

    let response: TokenResponse = serde_json::from_slice(&bytes).map_err(|error| {
        RefreshError::other(format!("Invalid Microsoft refresh response: {error}"))
    })?;
    if response.access_token.is_empty() {
        return Err(RefreshError::other(
            "Microsoft refresh response did not include an access token",
        ));
    }
    Ok(MicrosoftOAuthTokens {
        access_token: response.access_token,
        refresh_token: response.refresh_token,
        expires_in: response.expires_in,
        scope: response.scope,
        token_type: response.token_type,
        id_token: response.id_token,
    })
}

/// Renews a Minecraft session without opening a browser. The refresh token is
/// reused when Microsoft does not rotate it.
pub async fn refresh_minecraft_session<F>(
    config: &MicrosoftOAuthConfig,
    refresh_token: &str,
    progress: F,
) -> Result<MinecraftSession, RefreshError>
where
    F: FnMut(&str),
{
    let oauth = refresh_microsoft_tokens(config, refresh_token).await?;
    let mut session = complete_minecraft_login_with_progress(&oauth, progress)
        .await
        .map_err(RefreshError::other)?;
    if session.microsoft_refresh_token.is_none() {
        session.microsoft_refresh_token = Some(refresh_token.to_owned());
    }
    Ok(session)
}

/// Owns local account metadata, credential storage, and silent renewal.
#[derive(Clone)]
pub struct AuthManager {
    config: MicrosoftOAuthConfig,
    store: CredentialStore,
}

impl AuthManager {
    pub fn new(root: impl Into<PathBuf>, config: MicrosoftOAuthConfig) -> Self {
        Self {
            store: CredentialStore::new(root),
            config,
        }
    }

    pub fn config(&self) -> &MicrosoftOAuthConfig {
        &self.config
    }

    pub fn credential_backend(&self) -> &'static str {
        self.store.backend()
    }

    pub fn storage_warning(&self) -> Option<&str> {
        self.store.warning()
    }

    pub async fn accounts(&self) -> Result<Vec<AccountRecord>, String> {
        self.store.list_accounts().await
    }

    pub async fn status(&self) -> Result<serde_json::Value, String> {
        let accounts = self.store.list_accounts().await?;
        let default_account = self.store.default_account().await?;
        Ok(serde_json::json!({
            "signed_in": !accounts.is_empty(),
            "default_account": default_account,
            "credential_backend": self.store.backend(),
            "storage_warning": self.store.warning(),
            "accounts": accounts,
        }))
    }

    async fn acquire_lock(&self, account_id: &str) -> Result<std::fs::File, String> {
        let store = self.store.clone();
        let account_id = account_id.to_owned();
        tokio::task::spawn_blocking(move || store.lock_account(&account_id))
            .await
            .map_err(|e| format!("Could not acquire the account lock: {e}"))?
    }

    /// Persists a completed login. The refresh token is the only token written
    /// to disk or to the OS keyring.
    pub async fn save_session(
        &self,
        session: &MinecraftSession,
    ) -> Result<(AccountRecord, Option<String>), String> {
        let refresh_token = session
            .microsoft_refresh_token
            .as_deref()
            .ok_or("Microsoft did not return a refresh token; sign in again")?;
        let account_id = session.profile.id.clone();
        let lock = self.acquire_lock(&account_id).await?;
        let warning = self
            .store
            .save_refresh_token(&account_id, refresh_token)
            .await?;
        let record = AccountRecord {
            schema_version: 1,
            account_id: account_id.clone(),
            username: session.profile.name.clone(),
            last_login: crate::engine::timestamp_rfc3339(),
            credential_backend: Some(self.store.backend().to_owned()),
        };
        self.store.save_account(&record).await?;
        if self.store.default_account().await?.is_none() {
            self.store.set_default_account(&account_id).await?;
        }
        let _ = fs2::FileExt::unlock(&lock);
        Ok((record, warning))
    }

    async fn select_account(&self, account_id: Option<&str>) -> Result<String, String> {
        if let Some(account_id) = account_id {
            crate::credentials::validate_account_id(account_id)?;
            if self.store.load_account(account_id).await?.is_none() {
                return Err(format!("Unknown account: {account_id}"));
            }
            return Ok(account_id.to_owned());
        }
        if let Some(default) = self.store.default_account().await? {
            return Ok(default);
        }
        let accounts = self.store.list_accounts().await?;
        match accounts.as_slice() {
            [] => Err("No Microsoft account is signed in; run `umc auth login` first".into()),
            [only] => Ok(only.account_id.clone()),
            _ => Err("Multiple accounts are signed in; pass --account <account-id>".into()),
        }
    }

    /// Renews the selected account's Minecraft session without user
    /// interaction. Invalid refresh tokens are removed so the next login is a
    /// clean one.
    pub async fn refresh_session<F>(
        &self,
        account_id: Option<&str>,
        mut progress: F,
    ) -> Result<MinecraftSession, String>
    where
        F: FnMut(&str),
    {
        let account_id = self.select_account(account_id).await?;
        let lock = self.acquire_lock(&account_id).await?;
        let refresh_token = self
            .store
            .load_refresh_token(&account_id)
            .await?
            .ok_or_else(|| {
                "No stored credential for this account; run `umc auth login` again".to_string()
            })?;

        let session =
            match refresh_minecraft_session(&self.config, &refresh_token, &mut progress).await {
                Ok(session) => session,
                Err(error) if error.invalid_grant => {
                    self.store.delete_refresh_token(&account_id).await?;
                    let _ = self.store.delete_account(&account_id).await;
                    let _ = self.store.clear_default_account(&account_id).await;
                    let _ = fs2::FileExt::unlock(&lock);
                    return Err(format!("{}; run `umc auth login` again", error.message));
                }
                Err(error) => {
                    let _ = fs2::FileExt::unlock(&lock);
                    return Err(error.message);
                }
            };

        if let Some(token) = session.microsoft_refresh_token.as_deref() {
            self.store.save_refresh_token(&account_id, token).await?;
        }
        let record = AccountRecord {
            schema_version: 1,
            account_id: account_id.clone(),
            username: session.profile.name.clone(),
            last_login: crate::engine::timestamp_rfc3339(),
            credential_backend: Some(self.store.backend().to_owned()),
        };
        self.store.save_account(&record).await?;
        if self.store.default_account().await?.is_none() {
            self.store.set_default_account(&account_id).await?;
        }
        let _ = fs2::FileExt::unlock(&lock);
        Ok(session)
    }

    pub async fn logout(&self, account_id: Option<&str>) -> Result<String, String> {
        let account_id = self.select_account(account_id).await?;
        let lock = self.acquire_lock(&account_id).await?;
        self.store.delete_refresh_token(&account_id).await?;
        self.store.delete_account(&account_id).await?;
        self.store.clear_default_account(&account_id).await?;
        let _ = fs2::FileExt::unlock(&lock);
        Ok(account_id)
    }
}

/// Completes the Microsoft -> Xbox Live -> XSTS -> Minecraft login chain.
pub async fn complete_minecraft_login(
    oauth: &MicrosoftOAuthTokens,
) -> Result<MinecraftSession, String> {
    complete_minecraft_login_with_progress(oauth, |_| {}).await
}

/// Like [`complete_minecraft_login`], but reports each server-side stage.
pub async fn complete_minecraft_login_with_progress<F>(
    oauth: &MicrosoftOAuthTokens,
    mut progress: F,
) -> Result<MinecraftSession, String>
where
    F: FnMut(&str),
{
    let client = crate::engine::http_client()?;

    progress("Authenticating with Xbox Live");
    let xbl = authenticate_xbox_live(&client, &oauth.access_token).await?;

    progress("Authorizing with XSTS");
    let xsts = authorize_xsts(&client, &xbl.token).await?;

    progress("Logging in to Minecraft services");
    let minecraft = login_with_xbox(&client, &xsts.user_hash, &xsts.token).await?;
    let xuid = xsts.xuid.clone().or(xbl.xuid.clone());

    progress("Fetching Minecraft profile");
    let profile = fetch_minecraft_profile(&client, &minecraft.access_token).await?;

    Ok(MinecraftSession {
        microsoft_access_token: oauth.access_token.clone(),
        microsoft_refresh_token: oauth.refresh_token.clone(),
        microsoft_expires_in: oauth.expires_in,
        minecraft_access_token: minecraft.access_token,
        minecraft_expires_in: minecraft.expires_in,
        xuid,
        profile,
    })
}

async fn authenticate_xbox_live(
    client: &reqwest::Client,
    microsoft_access_token: &str,
) -> Result<XboxLiveAuth, String> {
    let body = serde_json::json!({
        "Properties": {
            "AuthMethod": "RPS",
            "SiteName": "user.auth.xboxlive.com",
            "RpsTicket": format!("d={microsoft_access_token}")
        },
        "RelyingParty": "http://auth.xboxlive.com",
        "TokenType": "JWT"
    });
    request_xbox_token(
        client,
        XBL_USER_AUTH_URL,
        &body,
        "Xbox Live user authentication",
    )
    .await
}

async fn authorize_xsts(client: &reqwest::Client, xbl_token: &str) -> Result<XboxLiveAuth, String> {
    let body = serde_json::json!({
        "Properties": {
            "SandboxId": "RETAIL",
            "UserTokens": [xbl_token]
        },
        "RelyingParty": XSTS_RELYING_PARTY,
        "TokenType": "JWT"
    });
    request_xbox_token(client, XSTS_AUTH_URL, &body, "XSTS authorization").await
}

async fn request_xbox_token(
    client: &reqwest::Client,
    url: &str,
    body: &serde_json::Value,
    step: &str,
) -> Result<XboxLiveAuth, String> {
    let response = client
        .post(url)
        .header("Accept", "application/json")
        .header("x-xbl-contract-version", "1")
        .json(body)
        .send()
        .await
        .map_err(|e| format!("{step} request failed: {e}"))?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Could not read the {step} response: {e}"))?;
    if !status.is_success() {
        return Err(xbox_error_message(step, status.as_u16(), &bytes));
    }

    let response: XboxTokenResponse =
        serde_json::from_slice(&bytes).map_err(|e| format!("Invalid {step} response: {e}"))?;
    let claim = response
        .display_claims
        .xui
        .first()
        .ok_or_else(|| format!("{step} response did not include a user hash"))?;
    Ok(XboxLiveAuth {
        token: response.token,
        user_hash: claim.uhs.clone(),
        xuid: claim.xid.clone(),
    })
}

fn xbox_error_message(step: &str, status: u16, body: &[u8]) -> String {
    let mut message = format!("{step} failed (HTTP {status})");
    if let Ok(error) = serde_json::from_slice::<XboxErrorResponse>(body) {
        if let Some(code) = error.xerr {
            message.push_str(&format!(", XErr {code}"));
            match code {
                2148916233 => message
                    .push_str(": this Microsoft account has no Xbox profile; create one first"),
                2148916235 => message
                    .push_str(": Xbox Live is not available in this account's country or region"),
                2148916236 => message.push_str(": this account requires adult verification"),
                2148916238 => message
                    .push_str(": this is a child account and must be added to a Microsoft family"),
                _ => {}
            }
        }
        if let Some(description) = error.message.filter(|description| !description.is_empty()) {
            message.push_str(&format!(" ({description})"));
        }
        if let Some(redirect) = error.redirect.filter(|redirect| !redirect.is_empty()) {
            message.push_str(&format!("; open {redirect} to resolve the account issue"));
        }
    }
    message
}

async fn login_with_xbox(
    client: &reqwest::Client,
    user_hash: &str,
    xsts_token: &str,
) -> Result<MinecraftLoginResponse, String> {
    let identity_token = format!("XBL3.0 x={user_hash};{xsts_token}");
    let response = client
        .post(MINECRAFT_LOGIN_URL)
        .header("Accept", "application/json")
        .json(&serde_json::json!({ "identityToken": identity_token }))
        .send()
        .await
        .map_err(|e| format!("Minecraft login request failed: {e}"))?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Could not read the Minecraft login response: {e}"))?;
    if !status.is_success() {
        let message = serde_json::from_slice::<MinecraftErrorResponse>(&bytes)
            .ok()
            .and_then(|error| error.error_message.or(error.error))
            .filter(|message| !message.is_empty())
            .unwrap_or_else(|| status.to_string());
        return Err(format!(
            "Minecraft login failed (HTTP {}): {message}",
            status.as_u16()
        ));
    }

    let response: MinecraftLoginResponse = serde_json::from_slice(&bytes)
        .map_err(|e| format!("Invalid Minecraft login response: {e}"))?;
    if response.access_token.is_empty() {
        return Err("Minecraft login response did not include an access token".into());
    }
    Ok(response)
}

async fn fetch_minecraft_profile(
    client: &reqwest::Client,
    minecraft_access_token: &str,
) -> Result<MinecraftProfile, String> {
    let response = client
        .get(MINECRAFT_PROFILE_URL)
        .header("Accept", "application/json")
        .bearer_auth(minecraft_access_token)
        .send()
        .await
        .map_err(|e| format!("Minecraft profile request failed: {e}"))?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_FOUND {
        return Err("This Microsoft account does not own Minecraft: Java Edition".into());
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("Could not read the Minecraft profile response: {e}"))?;
    if !status.is_success() {
        let message = serde_json::from_slice::<MinecraftErrorResponse>(&bytes)
            .ok()
            .and_then(|error| error.error_message.or(error.error))
            .filter(|message| !message.is_empty())
            .unwrap_or_else(|| status.to_string());
        return Err(format!(
            "Minecraft profile request failed (HTTP {}): {message}",
            status.as_u16()
        ));
    }
    serde_json::from_slice(&bytes).map_err(|e| format!("Invalid Minecraft profile response: {e}"))
}

fn open_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        for (program, arguments) in [("xdg-open", &[][..]), ("gio", &["open"][..])] {
            let mut command = Command::new(program);
            command
                .args(arguments)
                .arg(url)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            if command.spawn().is_ok() {
                return Ok(());
            }
        }
        Err("No browser opener found (tried xdg-open and gio open)".into())
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Could not open the browser: {e}"))
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Could not open the browser: {e}"))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = url;
        Err("Opening a browser is not supported on this platform".into())
    }
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    id_token: Option<String>,
}

#[derive(Deserialize)]
struct OAuthError {
    error: String,
    #[serde(default)]
    error_description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_challenge_matches_rfc7636_example() {
        assert_eq!(
            code_challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn authorization_url_contains_required_parameters() {
        let config = MicrosoftOAuthConfig::default();
        let pkce = Pkce::generate();
        let expected_challenge = code_challenge(&pkce.verifier);
        let raw_url = authorization_url(&config, &pkce).unwrap();
        assert!(raw_url.contains("/consumers/oauth2/v2.0/authorize"));
        assert!(raw_url.contains("scope=XboxLive.signin%20offline_access"));
        let url = Url::parse(&raw_url).unwrap();
        let params: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(
            params.get("client_id").map(String::as_str),
            Some(MICROSOFT_CLIENT_ID)
        );
        assert_eq!(
            params.get("redirect_uri").map(String::as_str),
            Some(DEFAULT_REDIRECT_URI)
        );
        assert_eq!(
            params.get("response_type").map(String::as_str),
            Some("code")
        );
        assert_eq!(
            params.get("code_challenge_method").map(String::as_str),
            Some("S256")
        );
        assert_eq!(
            params.get("state").map(String::as_str),
            Some(pkce.state.as_str())
        );
        assert_eq!(
            params.get("code_challenge").map(String::as_str),
            Some(expected_challenge.as_str())
        );
    }

    #[test]
    fn callback_accepts_code_and_validates_state() {
        let request =
            "GET /callback?code=abc123&state=expected HTTP/1.1\r\nHost: localhost:3000\r\n\r\n";
        assert_eq!(
            parse_callback_request(request, "expected", "/callback").unwrap(),
            Some("abc123".into())
        );
        assert!(parse_callback_request(request, "different", "/callback").is_err());
    }

    #[test]
    fn callback_reports_oauth_error() {
        let request =
            "GET /callback?error=access_denied&error_description=User%20cancelled HTTP/1.1\r\n\r\n";
        let error = parse_callback_request(request, "expected", "/callback").unwrap_err();
        assert!(error.contains("User cancelled"));
    }

    #[test]
    fn callback_ignores_other_paths() {
        let request = "GET /favicon.ico HTTP/1.1\r\n\r\n";
        assert_eq!(
            parse_callback_request(request, "expected", "/callback").unwrap(),
            None
        );
    }

    #[test]
    fn redirect_settings_accept_loopback_with_optional_port() {
        assert_eq!(
            redirect_settings("http://localhost").unwrap(),
            ("localhost".into(), None, "/".into())
        );
        assert_eq!(
            redirect_settings("http://localhost:3000/callback").unwrap(),
            ("localhost".into(), Some(3000), "/callback".into())
        );
        assert!(redirect_settings("https://localhost").is_err());
        assert!(redirect_settings("http://example.com:3000/callback").is_err());
    }

    #[tokio::test]
    async fn login_flow_uses_a_dynamic_loopback_port() {
        let config = MicrosoftOAuthConfig::default();
        let flow = LoginFlow::start_with_config(config, false).await.unwrap();
        assert!(flow.redirect_uri().starts_with("http://localhost:"));
        assert!(
            flow.auth_url()
                .contains("redirect_uri=http%3A%2F%2Flocalhost%3A")
        );
    }

    #[test]
    fn minecraft_profile_deserializes() {
        let profile: MinecraftProfile = serde_json::from_str(
            r#"{"id":"069a79f444e94726a5befca90e38aaf5","name":"Steve","skins":[],"capes":[]}"#,
        )
        .unwrap();
        assert_eq!(profile.id, "069a79f444e94726a5befca90e38aaf5");
        assert_eq!(profile.name, "Steve");
    }

    #[test]
    fn xbox_error_maps_missing_profile_code() {
        let body = br#"{"XErr":2148916233,"Message":"","Redirect":"https://example.test/xbox"}"#;
        let message = xbox_error_message("Xbox Live user authentication", 401, body);
        assert!(message.contains("2148916233"));
        assert!(message.contains("no Xbox profile"));
        assert!(message.contains("https://example.test/xbox"));
    }

    #[test]
    fn minecraft_login_response_deserializes() {
        let response: MinecraftLoginResponse = serde_json::from_str(
            r#"{"username":"069a79f444e94726a5befca90e38aaf5","access_token":"token","expires_in":86400}"#,
        )
        .unwrap();
        assert_eq!(response.access_token, "token");
        assert_eq!(response.expires_in, Some(86400));
    }

    #[test]
    fn session_debug_redacts_tokens() {
        let session = MinecraftSession {
            microsoft_access_token: "ms-access-secret".into(),
            microsoft_refresh_token: Some("ms-refresh-secret".into()),
            microsoft_expires_in: Some(3600),
            minecraft_access_token: "mc-access-secret".into(),
            minecraft_expires_in: Some(86400),
            xuid: Some("2535400000000000".into()),
            profile: MinecraftProfile {
                id: "069a79f444e94726a5befca90e38aaf5".into(),
                name: "Steve".into(),
                skins: serde_json::json!([]),
                capes: serde_json::json!([]),
            },
        };
        let debug = format!("{session:?}");
        assert!(!debug.contains("ms-access-secret"));
        assert!(!debug.contains("ms-refresh-secret"));
        assert!(!debug.contains("mc-access-secret"));
        assert!(debug.contains("Steve"));
    }

    #[tokio::test]
    async fn auth_manager_persists_and_logs_out_an_account() {
        let root = std::env::temp_dir().join(format!("umc-auth-{}", Uuid::new_v4()));
        let manager = AuthManager {
            config: MicrosoftOAuthConfig::default(),
            store: crate::credentials::CredentialStore::file_store(root.clone()),
        };
        let session = MinecraftSession {
            microsoft_access_token: "ms-access-secret".into(),
            microsoft_refresh_token: Some("ms-refresh-secret".into()),
            microsoft_expires_in: Some(3600),
            minecraft_access_token: "mc-access-secret".into(),
            minecraft_expires_in: Some(86400),
            xuid: Some("2535400000000000".into()),
            profile: MinecraftProfile {
                id: "069a79f444e94726a5befca90e38aaf5".into(),
                name: "Steve".into(),
                skins: serde_json::json!([]),
                capes: serde_json::json!([]),
            },
        };
        let (record, warning) = manager.save_session(&session).await.unwrap();
        assert!(warning.is_none());
        assert_eq!(record.username, "Steve");
        assert_eq!(
            manager
                .store
                .load_refresh_token(&session.profile.id)
                .await
                .unwrap()
                .as_deref(),
            Some("ms-refresh-secret")
        );
        let status = manager.status().await.unwrap();
        assert_eq!(status["signed_in"], true);
        assert_eq!(manager.accounts().await.unwrap().len(), 1);

        manager.logout(None).await.unwrap();
        assert!(manager.accounts().await.unwrap().is_empty());
        assert!(
            manager
                .store
                .load_refresh_token(&session.profile.id)
                .await
                .unwrap()
                .is_none()
        );
        let _ = tokio::fs::remove_dir_all(root).await;
    }
}
