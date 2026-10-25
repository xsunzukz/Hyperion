use crate::error::{AppError, Result};
use crate::models::{GameAsset, GameMetadata, Session};
use base64::Engine;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, USER_AGENT};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

pub const DEFAULT_CLIENT_ID: &str = "34a02cf8f4414e29b15921876da36f9a";
pub const DEFAULT_CLIENT_SECRET: &str = "daafbccc737745039dffe53d94fc76cf";
pub const USER_AGENT_STRING: &str =
    "UELauncher/11.0.1-14907503+++Portal+Release-Live Windows/10.0.19041.1.256.64bit";

pub const OAUTH_HOST: &str = "account-public-service-prod03.ol.epicgames.com";
pub const LAUNCHER_HOST: &str = "launcher-public-service-prod06.ol.epicgames.com";
pub const ENTITLEMENTS_HOST: &str = "entitlement-public-service-prod08.ol.epicgames.com";
pub const CATALOG_HOST: &str = "catalog-public-service-prod06.ol.epicgames.com";
pub const ECOMMERCE_HOST: &str = "ecommerceintegration-public-service-ecomprod02.ol.epicgames.com";
pub const DATASTORAGE_HOST: &str = "datastorage-public-service-liveegs.live.use1a.on.epicgames.com";

/// User agent for requests to anyone other than Epic.
pub const HONEST_USER_AGENT: &str = concat!("Hyperion/", env!("CARGO_PKG_VERSION"));

/// Epic's download CDNs (manifests and chunks), which see the launcher UA.
fn is_epic_cdn_host(url: &str) -> bool {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
        .is_some_and(|host| {
            host.ends_with(".akamaized.net")
                || host.ends_with(".fastly-edge.com")
                || host.ends_with(".epicgamescdn.com")
        })
}

/// True only for Epic's own hosts, the one place the account token may go.
///
/// Matched on the parsed host: a substring test on the whole URL handed the
/// token to any mirror whose path merely contained "epicgames".
pub fn is_epic_host(url: &str) -> bool {
    url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_ascii_lowercase))
        .is_some_and(|host| host == "epicgames.com" || host.ends_with(".epicgames.com"))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestApiEntry {
    pub uri: String,
    #[serde(rename = "queryParams")]
    pub query_params: Option<Vec<ManifestQueryParam>>,
}

impl ManifestApiEntry {
    pub fn download_url(&self) -> String {
        if let Some(ref params) = self.query_params {
            if !params.is_empty() {
                let query_str = params
                    .iter()
                    .map(|p| format!("{}={}", p.name, p.value))
                    .collect::<Vec<_>>()
                    .join("&");
                if self.uri.contains('?') {
                    return format!("{}&{}", self.uri, query_str);
                } else {
                    return format!("{}?{}", self.uri, query_str);
                }
            }
        }
        self.uri.clone()
    }

    pub fn base_url(&self) -> String {
        self.uri
            .rfind('/')
            .map(|idx| self.uri[..idx].to_string())
            .unwrap_or_else(|| self.uri.clone())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestQueryParam {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestApiElement {
    pub app_name: Option<String>,
    pub hash: String,
    pub manifests: Vec<ManifestApiEntry>,
    #[serde(rename = "isPreloaded")]
    pub is_preloaded: Option<bool>,
    pub secrets: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestApiResponse {
    pub elements: Vec<ManifestApiElement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExchangeCodeResponse {
    pub code: String,
    #[serde(rename = "creatingClientId")]
    pub creating_client_id: Option<String>,
    #[serde(rename = "expiresInSeconds")]
    pub expires_in_seconds: Option<i64>,
}

type SessionSink = Box<dyn Fn(&Session) + Send + Sync>;

/// Renew the access token when less than this much of it is left.
const REFRESH_MARGIN_SECS: i64 = 300;

/// True when the session's access token expires within the margin. A session
/// without a readable expiry is left alone rather than refreshed on every call.
pub fn needs_refresh(session: &Session, now: chrono::DateTime<chrono::Utc>) -> bool {
    session
        .expires_at
        .as_deref()
        .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
        .is_some_and(|at| at.with_timezone(&chrono::Utc) - now < chrono::Duration::seconds(REFRESH_MARGIN_SECS))
}

pub struct EpicApiClient {
    client: Client,
    session: Arc<RwLock<Option<Session>>>,
    client_id: String,
    client_secret: String,
    refresh_lock: tokio::sync::Mutex<()>,
    session_sink: std::sync::RwLock<Option<SessionSink>>,
}

impl Default for EpicApiClient {
    fn default() -> Self {
        Self::new()
    }
}

impl EpicApiClient {
    pub fn new() -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_STRING));

        let client = Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            session: Arc::new(RwLock::new(None)),
            client_id: DEFAULT_CLIENT_ID.to_string(),
            client_secret: DEFAULT_CLIENT_SECRET.to_string(),
            refresh_lock: tokio::sync::Mutex::new(()),
            session_sink: std::sync::RwLock::new(None),
        }
    }

    pub async fn set_session(&self, session: Option<Session>) {
        let mut s = self.session.write().await;
        *s = session;
    }

    pub async fn get_session(&self) -> Option<Session> {
        let s = self.session.read().await;
        s.clone()
    }

    fn basic_auth_header(&self) -> String {
        let creds = format!("{}:{}", self.client_id, self.client_secret);
        let encoded = base64::engine::general_purpose::STANDARD.encode(creds);
        format!("Basic {}", encoded)
    }

    pub fn get_login_url(&self) -> String {
        let redirect_target = format!(
            "https://www.epicgames.com/id/api/redirect?clientId={}&responseType=code",
            self.client_id
        );
        let encoded_redirect =
            url::form_urlencoded::byte_serialize(redirect_target.as_bytes()).collect::<String>();
        format!(
            "https://www.epicgames.com/id/login?redirectUrl={}",
            encoded_redirect
        )
    }

    pub async fn request_oauth_token(&self, params: &[(&str, &str)]) -> Result<Session> {
        let url = format!("https://{}/account/api/oauth/token", OAUTH_HOST);
        debug!("Requesting OAuth token with grant parameters");

        let resp = self
            .client
            .post(&url)
            .header(AUTHORIZATION, self.basic_auth_header())
            .form(&params)
            .send()
            .await?;

        let status = resp.status();
        let body_text = resp.text().await?;

        if !status.is_success() {
            error!("OAuth token request failed ({}): {}", status, body_text);
            if let Ok(err_json) = serde_json::from_str::<serde_json::Value>(&body_text) {
                let code = err_json["errorCode"].as_str().unwrap_or("unknown_error");
                let msg = err_json["errorMessage"].as_str().unwrap_or(&body_text);
                return Err(AppError::Api {
                    code: code.to_string(),
                    message: msg.to_string(),
                });
            }
            return Err(AppError::Auth(format!(
                "OAuth failed with status {}: {}",
                status, body_text
            )));
        }

        let mut session: Session = serde_json::from_str(&body_text)?;

        // Calculate expires_at if absent
        if session.expires_at.is_none() {
            let now = chrono::Utc::now();
            let exp = now + chrono::Duration::seconds(session.expires_in);
            session.expires_at = Some(exp.to_rfc3339());
        }

        self.set_session(Some(session.clone())).await;
        Ok(session)
    }

    pub async fn login_with_code(&self, auth_code: &str) -> Result<Session> {
        let clean_code = auth_code.trim();
        info!("Logging in with authorization code");
        self.request_oauth_token(&[
            ("grant_type", "authorization_code"),
            ("code", clean_code),
            ("token_type", "eg1"),
        ])
        .await
    }

    pub async fn login_with_exchange_code(&self, exchange_code: &str) -> Result<Session> {
        let clean_code = exchange_code.trim();
        info!("Logging in with exchange code");
        self.request_oauth_token(&[
            ("grant_type", "exchange_code"),
            ("exchange_code", clean_code),
            ("token_type", "eg1"),
        ])
        .await
    }

    pub async fn refresh_session(&self, refresh_token: &str) -> Result<Session> {
        info!("Refreshing OAuth session with refresh token");
        self.request_oauth_token(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("token_type", "eg1"),
        ])
        .await
    }

    pub async fn verify_session(&self, access_token: &str) -> Result<bool> {
        let url = format!("https://{}/account/api/oauth/verify", OAUTH_HOST);
        let resp = self
            .client
            .get(&url)
            .header(AUTHORIZATION, format!("bearer {}", access_token))
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(true)
        } else {
            warn!("Session verification returned status {}", resp.status());
            Ok(false)
        }
    }

    async fn get_auth_token(&self) -> Result<String> {
        Ok(self.fresh_session().await?.access_token)
    }

    /// The signed-in session, renewed first when its access token is about to
    /// expire.
    ///
    /// Hyperion lives in the tray for days, but the token used to be renewed
    /// only at startup, so every Epic call (library refresh, installs, launch)
    /// started failing a few hours in. Concurrent callers share one refresh. A
    /// failed refresh hands back the old session and the call reports Epic's
    /// own error.
    pub async fn fresh_session(&self) -> Result<Session> {
        let not_signed_in = || AppError::Auth("User is not logged in".to_string());
        let current = self.get_session().await.ok_or_else(not_signed_in)?;
        if !needs_refresh(&current, chrono::Utc::now()) {
            return Ok(current);
        }

        let _guard = self.refresh_lock.lock().await;
        // Someone else may have renewed it while this call waited
        let current = self.get_session().await.ok_or_else(not_signed_in)?;
        if !needs_refresh(&current, chrono::Utc::now()) {
            return Ok(current);
        }
        let Some(refresh_token) = current.refresh_token.clone() else {
            return Ok(current);
        };

        match self.refresh_session(&refresh_token).await {
            Ok(renewed) => {
                info!("Renewed the Epic session before it expired");
                if let Some(sink) = self.session_sink.read().ok().as_deref().and_then(Option::as_ref) {
                    sink(&renewed);
                }
                Ok(renewed)
            }
            Err(e) => {
                warn!("Could not renew the Epic session: {}", e);
                Ok(current)
            }
        }
    }

    /// Called with every renewed session, so the stored copy stays current.
    pub fn set_session_sink(&self, sink: impl Fn(&Session) + Send + Sync + 'static) {
        if let Ok(mut slot) = self.session_sink.write() {
            *slot = Some(Box::new(sink));
        }
    }

    pub async fn get_game_assets(&self, platform: &str) -> Result<Vec<GameAsset>> {
        let token = self.get_auth_token().await?;
        let url = format!(
            "https://{}/launcher/api/public/assets/{}",
            LAUNCHER_HOST, platform
        );
        debug!("Fetching assets for platform: {}", platform);

        let resp = self
            .client
            .get(&url)
            .query(&[("label", "Live")])
            .header(AUTHORIZATION, format!("bearer {}", token))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(AppError::Api {
                code: status.to_string(),
                message: err_text,
            });
        }

        let assets: Vec<GameAsset> = resp.json().await?;
        Ok(assets)
    }

    pub async fn get_user_entitlements(&self) -> Result<Vec<serde_json::Value>> {
        let (token, account_id) = {
            let session = self.fresh_session().await?;
            (session.access_token, session.account_id)
        };

        let mut all_entitlements = Vec::new();
        let mut start = 0;

        loop {
            let url = format!(
                "https://{}/entitlement/api/account/{}/entitlements",
                ENTITLEMENTS_HOST, account_id
            );

            let resp = self
                .client
                .get(&url)
                .query(&[("start", start.to_string()), ("count", "1000".to_string())])
                .header(AUTHORIZATION, format!("bearer {}", token))
                .send()
                .await?;

            if !resp.status().is_success() {
                break;
            }

            let batch: Vec<serde_json::Value> = resp.json().await?;
            let len = batch.len();
            all_entitlements.extend(batch);
            if len < 1000 {
                break;
            }
            start += len;
        }

        Ok(all_entitlements)
    }

    pub async fn get_catalog_item(
        &self,
        namespace: &str,
        catalog_item_id: &str,
    ) -> Result<Option<GameMetadata>> {
        let token = self.get_auth_token().await?;
        let url = format!(
            "https://{}/catalog/api/shared/namespace/{}/bulk/items",
            CATALOG_HOST, namespace
        );

        let resp = self
            .client
            .get(&url)
            .query(&[
                ("id", catalog_item_id),
                ("includeDLCDetails", "true"),
                ("includeMainGameDetails", "true"),
                ("country", "US"),
                ("locale", "en"),
            ])
            .header(AUTHORIZATION, format!("bearer {}", token))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(None);
        }

        let map: HashMap<String, GameMetadata> = resp.json().await?;
        Ok(map.into_values().next())
    }

    pub async fn get_manifest_response(
        &self,
        namespace: &str,
        catalog_item_id: &str,
        app_name: &str,
        platform: &str,
    ) -> Result<ManifestApiResponse> {
        let token = self.get_auth_token().await?;
        let url = format!(
            "https://{}/launcher/api/public/assets/v2/platform/{}/namespace/{}/catalogItem/{}/app/{}/label/Live",
            LAUNCHER_HOST, platform, namespace, catalog_item_id, app_name
        );

        let resp = self
            .client
            .get(&url)
            .header(AUTHORIZATION, format!("bearer {}", token))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api {
                code: status.to_string(),
                message: format!("Manifest API returned {}: {}", status, body),
            });
        }

        let manifest_resp: ManifestApiResponse = resp.json().await?;
        Ok(manifest_resp)
    }

    /// Resolves the CDN manifest info for an app by its app name alone.
    ///
    /// Third-party APIs sometimes mirror only a `.manifest` file while the game
    /// data itself still lives on Epic's CDN; this lets the downloader find the
    /// real chunk host without a catalog entry.
    pub async fn get_manifest_response_by_app(
        &self,
        platform: &str,
        app_name: &str,
    ) -> Result<ManifestApiResponse> {
        let token = self.get_auth_token().await?;
        let url = format!(
            "https://{}/launcher/api/public/assets/{}/{}?label=Live",
            LAUNCHER_HOST, platform, app_name
        );

        let resp = self
            .client
            .get(&url)
            .header(AUTHORIZATION, format!("bearer {}", token))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Api {
                code: status.to_string(),
                message: format!("Assets API returned {} for {}: {}", status, app_name, body),
            });
        }

        Ok(resp.json().await?)
    }

    pub async fn download_manifest_from_element(
        &self,
        element: &ManifestApiElement,
    ) -> Result<(Vec<u8>, Vec<String>)> {
        if element.manifests.is_empty() {
            return Err(AppError::Manifest(
                "No manifest entries in element".to_string(),
            ));
        }

        let mut base_urls = Vec::new();
        for m in &element.manifests {
            let bu = m.base_url();
            if !base_urls.contains(&bu) {
                base_urls.push(bu);
            }
        }

        let mut last_error = String::new();
        for (i, entry) in element.manifests.iter().enumerate() {
            let url = entry.download_url();
            info!(
                "Attempting manifest download from CDN mirror {}/{}: {}",
                i + 1,
                element.manifests.len(),
                entry.uri
            );
            match self.client.get(&url).send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let bytes = resp.bytes().await?.to_vec();
                        info!(
                            "Successfully downloaded manifest binary ({} bytes) from mirror {}",
                            bytes.len(),
                            i + 1
                        );
                        return Ok((bytes, base_urls));
                    } else {
                        warn!("CDN mirror {} returned HTTP {}: {}", i + 1, status, url);
                        last_error = format!("HTTP {} from {}", status, url);
                    }
                }
                Err(e) => {
                    warn!("CDN mirror {} network error: {}", i + 1, e);
                    last_error = format!("Network error: {}", e);
                }
            }
        }

        Err(AppError::Network(format!(
            "Failed to download manifest from all {} CDN mirrors. Last failure: {}",
            element.manifests.len(),
            last_error
        )))
    }

    pub async fn download_manifest_bytes(&self, uri: &str) -> Result<Vec<u8>> {
        debug!("Downloading manifest binary from: {}", uri);
        let mut request = self.client.get(uri);
        // The Epic launcher user agent is for Epic. Community mirrors get an
        // honest one rather than a client pretending to be Epic's launcher.
        if !is_epic_host(uri) && !is_epic_cdn_host(uri) {
            request = request.header(USER_AGENT, HONEST_USER_AGENT);
        }
        let resp = request.send().await?;
        if !resp.status().is_success() {
            return Err(AppError::Network(format!(
                "Failed to download manifest from {}: status {}",
                uri,
                resp.status()
            )));
        }
        let bytes = resp.bytes().await?;
        Ok(bytes.to_vec())
    }

    pub async fn get_exchange_code_for_launcher(&self) -> Result<String> {
        let token = self.get_auth_token().await?;
        let url = format!("https://{}/account/api/oauth/exchange", OAUTH_HOST);

        let resp = self
            .client
            .get(&url)
            .header(AUTHORIZATION, format!("bearer {}", token))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(AppError::Auth(format!(
                "Failed to generate exchange code ({}): {}",
                status, body
            )));
        }

        let ex: ExchangeCodeResponse = resp.json().await?;
        Ok(ex.code)
    }

    pub async fn get_ownership_token(
        &self,
        namespace: &str,
        catalog_item_id: &str,
    ) -> Result<Vec<u8>> {
        let (token, account_id) = {
            let session = self.fresh_session().await?;
            (session.access_token, session.account_id)
        };

        let url = format!(
            "https://{}/ecommerceintegration/api/public/platforms/EPIC/identities/{}/ownershipToken",
            ECOMMERCE_HOST, account_id
        );

        let ns_item = format!("{}:{}", namespace, catalog_item_id);
        let resp = self
            .client
            .post(&url)
            .header(AUTHORIZATION, format!("bearer {}", token))
            .form(&[("nsCatalogItemId", &ns_item)])
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(AppError::Api {
                code: resp.status().to_string(),
                message: "Failed to get ownership token".to_string(),
            });
        }

        let bytes = resp.bytes().await?;
        Ok(bytes.to_vec())
    }

    pub async fn get_cloud_save_manifests(&self, app_name: &str) -> Result<serde_json::Value> {
        let (token, account_id) = {
            let session = self.fresh_session().await?;
            (session.access_token, session.account_id)
        };

        let url = format!(
            "https://{}/api/v1/access/egstore/savesync/{}/{}/manifests/",
            DATASTORAGE_HOST, account_id, app_name
        );

        let resp = self
            .client
            .get(&url)
            .header(AUTHORIZATION, format!("bearer {}", token))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(serde_json::json!({ "files": [] }));
        }

        let val: serde_json::Value = resp
            .json()
            .await
            .unwrap_or(serde_json::json!({ "files": [] }));
        Ok(val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session_expiring_at(expires_at: Option<&str>) -> Session {
        serde_json::from_value(serde_json::json!({
            "access_token": "a",
            "expires_in": 7200,
            "expires_at": expires_at,
            "token_type": "bearer",
            "refresh_token": "r",
            "account_id": "acc",
        }))
        .unwrap()
    }

    #[test]
    fn sessions_renew_shortly_before_they_expire() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-10-25T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);

        assert!(!needs_refresh(&session_expiring_at(Some("2026-10-25T14:00:00.000Z")), now));
        assert!(needs_refresh(&session_expiring_at(Some("2026-10-25T12:04:00.000Z")), now));
        assert!(needs_refresh(&session_expiring_at(Some("2026-10-25T08:00:00Z")), now));
        assert!(needs_refresh(&session_expiring_at(Some("2026-10-25T17:31:00+05:30")), now));
        // Unknown expiry: leave it alone instead of refreshing on every call
        assert!(!needs_refresh(&session_expiring_at(None), now));
        assert!(!needs_refresh(&session_expiring_at(Some("garbage")), now));
    }

    #[test]
    fn token_goes_to_epic_hosts_only() {
        assert!(is_epic_host("https://download.epicgames.com/Builds/x.chunk"));
        assert!(is_epic_host("https://EPICGAMES.com/a"));
        assert!(is_epic_host("https://launcher-public-service-prod06.ol.epicgames.com/x"));

        assert!(!is_epic_host("https://mirror.example/epicgames/Builds/x.chunk"));
        assert!(!is_epic_host("https://epicgames.com.evil.example/x"));
        assert!(!is_epic_host("https://notepicgames.com/x"));
        assert!(!is_epic_host("https://api.mysticfox.dev/v2/epicgames.com/x"));
        assert!(!is_epic_host("not a url"));
    }
}
