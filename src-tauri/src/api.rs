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

pub struct EpicApiClient {
    client: Client,
    session: Arc<RwLock<Option<Session>>>,
    client_id: String,
    client_secret: String,
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
        let s = self.session.read().await;
        if let Some(ref session) = *s {
            Ok(session.access_token.clone())
        } else {
            Err(AppError::Auth("User is not logged in".to_string()))
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
            let s = self.session.read().await;
            if let Some(ref session) = *s {
                (session.access_token.clone(), session.account_id.clone())
            } else {
                return Err(AppError::Auth("Not logged in".to_string()));
            }
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
        let resp = self.client.get(uri).send().await?;
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
            let s = self.session.read().await;
            if let Some(ref session) = *s {
                (session.access_token.clone(), session.account_id.clone())
            } else {
                return Err(AppError::Auth("Not logged in".to_string()));
            }
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
            let s = self.session.read().await;
            if let Some(ref session) = *s {
                (session.access_token.clone(), session.account_id.clone())
            } else {
                return Err(AppError::Auth("Not logged in".to_string()));
            }
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
