use anyhow::Context;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::Rng;
use secrecy::{ExposeSecret, SecretBox};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcDiscoveryDoc {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: Option<String>,
    pub jwks_uri: Option<String>,
    pub end_session_endpoint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PkcePair {
    pub code_verifier: String,
    pub code_challenge: String,
}

pub fn generate_pkce_pair() -> PkcePair {
    let mut random_bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut random_bytes);
    let code_verifier = URL_SAFE_NO_PAD.encode(random_bytes);

    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();
    let code_challenge = URL_SAFE_NO_PAD.encode(hash);

    PkcePair {
        code_verifier,
        code_challenge,
    }
}

pub fn generate_random_token(bytes_len: usize) -> String {
    let mut random_bytes = vec![0u8; bytes_len];
    rand::rng().fill_bytes(&mut random_bytes);
    URL_SAFE_NO_PAD.encode(random_bytes)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OidcTokenResponse {
    pub access_token: String,
    pub token_type: Option<String>,
    pub expires_in: Option<i64>,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserInfoClaims {
    pub sub: Option<String>,
    pub name: Option<String>,
    pub preferred_username: Option<String>,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
    pub roles: Option<HashSet<String>>,
    pub groups: Option<HashSet<String>>,
}

pub struct OidcClient {
    client: reqwest::Client,
    discovery_cache: Arc<RwLock<Option<OidcDiscoveryDoc>>>,
}

impl Default for OidcClient {
    fn default() -> Self {
        Self::new()
    }
}

impl OidcClient {
    pub fn new() -> Self {
        OidcClient {
            client: reqwest::Client::new(),
            discovery_cache: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn get_discovery(&self, issuer_url: &str, discovery_url: Option<&str>) -> Result<OidcDiscoveryDoc, anyhow::Error> {
        {
            let cache = self.discovery_cache.read().await;
            if let Some(ref doc) = *cache {
                return Ok(doc.clone());
            }
        }

        let url = if let Some(disc) = discovery_url {
            disc.to_string()
        } else {
            let base = issuer_url.trim_end_matches('/');
            format!("{base}/.well-known/openid-configuration")
        };

        let res = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch OIDC discovery document")?;

        let status = res.status();
        if !status.is_success() {
            let text = res.text().await.unwrap_or_default();
            anyhow::bail!("OIDC discovery endpoint returned status {status}: {text}");
        }

        let doc: OidcDiscoveryDoc = res
            .json()
            .await
            .context("Failed to parse OIDC discovery document")?;

        let mut cache = self.discovery_cache.write().await;
        *cache = Some(doc.clone());
        Ok(doc)
    }

    pub fn build_authorization_url(
        auth_endpoint: &str,
        client_id: &str,
        redirect_uri: &str,
        scopes: &str,
        state: &str,
        nonce: &str,
        code_challenge: &str,
    ) -> Result<String, anyhow::Error> {
        let mut url = Url::parse(auth_endpoint).context("Invalid authorization endpoint URL")?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("response_type", "code");
            query.append_pair("client_id", client_id);
            query.append_pair("redirect_uri", redirect_uri);
            query.append_pair("scope", scopes);
            query.append_pair("state", state);
            query.append_pair("nonce", nonce);
            query.append_pair("code_challenge", code_challenge);
            query.append_pair("code_challenge_method", "S256");
        }
        Ok(url.to_string())
    }

    pub async fn exchange_code(
        &self,
        token_endpoint: &str,
        client_id: &str,
        client_secret: Option<&SecretBox<String>>,
        code: &str,
        code_verifier: &str,
        redirect_uri: &str,
    ) -> Result<OidcTokenResponse, anyhow::Error> {
        let mut params = vec![
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", code_verifier),
            ("client_id", client_id),
        ];

        let mut req = self.client.post(token_endpoint).form(&params);

        if let Some(secret) = client_secret {
            req = req.basic_auth(client_id, Some(secret.expose_secret()));
            params.push(("client_secret", secret.expose_secret().as_str()));
        }

        let res = req
            .form(&params)
            .send()
            .await
            .context("Failed to send token exchange request")?;

        let status = res.status();
        if !status.is_success() {
            let text = res.text().await.unwrap_or_default();
            anyhow::bail!("OIDC token endpoint returned status {status}: {text}");
        }

        let token_res: OidcTokenResponse = res
            .json()
            .await
            .context("Failed to parse token response")?;

        Ok(token_res)
    }

    pub async fn fetch_userinfo_raw(
        &self,
        userinfo_endpoint: &str,
        access_token: &str,
    ) -> Result<(UserInfoClaims, serde_json::Value), anyhow::Error> {
        let res = self
            .client
            .get(userinfo_endpoint)
            .bearer_auth(access_token)
            .send()
            .await
            .context("Failed to fetch userinfo")?;

        let status = res.status();
        if !status.is_success() {
            let text = res.text().await.unwrap_or_default();
            anyhow::bail!("OIDC userinfo endpoint returned status {status}: {text}");
        }

        let val: serde_json::Value = res.json().await.context("Failed to parse userinfo JSON")?;
        let claims = extract_claims_from_json(&val);
        Ok((claims, val))
    }

    pub async fn fetch_userinfo(
        &self,
        userinfo_endpoint: &str,
        access_token: &str,
    ) -> Result<UserInfoClaims, anyhow::Error> {
        let (claims, _) = self.fetch_userinfo_raw(userinfo_endpoint, access_token).await?;
        Ok(claims)
    }
}

pub fn decode_jwt_payload_unverified(jwt: &str) -> Option<serde_json::Value> {
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() < 2 {
        return None;
    }
    let payload_b64 = parts[1];
    let decoded = URL_SAFE_NO_PAD.decode(payload_b64).ok()?;
    serde_json::from_slice(&decoded).ok()
}

pub fn extract_claims_from_json(val: &serde_json::Value) -> UserInfoClaims {
    let sub = val.get("sub").and_then(|v| v.as_str()).map(ToString::to_string);
    let name = val.get("name").and_then(|v| v.as_str()).map(ToString::to_string);
    let preferred_username = val
        .get("preferred_username")
        .or_else(|| val.get("nickname"))
        .or_else(|| val.get("upn"))
        .or_else(|| val.get("unique_name"))
        .or_else(|| val.get("cognito:username"))
        .or_else(|| val.get("username"))
        .and_then(|v| v.as_str())
        .map(ToString::to_string);
    let email = val.get("email").and_then(|v| v.as_str()).map(ToString::to_string);
    let email_verified = val.get("email_verified").and_then(serde_json::Value::as_bool);

    let mut roles = HashSet::new();
    if let Some(arr) = val.get("roles").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(r) = item.as_str() {
                roles.insert(r.to_string());
            }
        }
    }
    if let Some(realm_access) = val.get("realm_access")
        && let Some(arr) = realm_access.get("roles").and_then(|v| v.as_array())
    {
        for item in arr {
            if let Some(r) = item.as_str() {
                roles.insert(r.to_string());
            }
        }
    }
    if let Some(resource_access) = val.get("resource_access").and_then(|v| v.as_object()) {
        for client in resource_access.values() {
            if let Some(arr) = client.get("roles").and_then(|v| v.as_array()) {
                for item in arr {
                    if let Some(r) = item.as_str() {
                        roles.insert(r.to_string());
                    }
                }
            }
        }
    }

    let mut groups = HashSet::new();
    if let Some(arr) = val
        .get("groups")
        .or_else(|| val.get("cognito:groups"))
        .and_then(|v| v.as_array())
    {
        for item in arr {
            if let Some(g) = item.as_str() {
                groups.insert(g.to_string());
                roles.insert(g.to_string());
            }
        }
    }

    UserInfoClaims {
        sub,
        name,
        preferred_username,
        email,
        email_verified,
        roles: Some(roles),
        groups: Some(groups),
    }
}
