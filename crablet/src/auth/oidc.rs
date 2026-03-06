use anyhow::{Result, Context};
use openid::{Client, Options, DiscoveredClient};
use std::sync::Arc;
use url::Url;

#[derive(Clone)]
pub struct OidcProvider {
    pub client: Arc<Client>,
}

impl OidcProvider {
    pub async fn discover(issuer_url: &str, client_id: &str, client_secret: &str, redirect_uri: &str) -> Result<Self> {
        let client = DiscoveredClient::discover(
            client_id.to_string(),
            client_secret.to_string(),
            Some(redirect_uri.to_string()),
            Url::parse(issuer_url).context("Invalid issuer URL")?,
        ).await.map_err(|e| anyhow::anyhow!("OIDC Discovery failed: {}", e))?;

        Ok(Self {
            client: Arc::new(client),
        })
    }

    pub fn get_authorization_url(&self) -> Url {
        self.client.auth_url(&Options {
            scope: Some("openid profile email".to_string()),
            ..Default::default()
        })
    }

    pub async fn exchange_code(&self, code: &str) -> Result<openid::Bearer> {
        let token = self.client.request_token(code).await.map_err(|e| anyhow::anyhow!("Token exchange failed: {}", e))?;
        Ok(token)
    }
}
