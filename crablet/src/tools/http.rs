use crate::plugins::Plugin;
use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::{redirect, Client};
use serde_json::Value;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::time::Duration;
use tokio::net::lookup_host;
use url::{Host, Url};

pub struct HttpPlugin;

#[async_trait]
impl Plugin for HttpPlugin {
    fn name(&self) -> &str {
        "read_url"
    }

    fn description(&self) -> &str {
        "Read the content of a URL. Args: {\"url\": \"...\"}"
    }

    async fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    async fn execute(&self, _command: &str, args: Value) -> Result<String> {
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .context("Missing 'url' argument")?;

        HttpTool::read_url(url).await
    }

    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct HttpTool;

impl HttpTool {
    pub async fn read_url(url: &str) -> Result<String> {
        let url = validate_outbound_url(url).await?;
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent("Crablet/0.1.0")
            .redirect(redirect::Policy::none())
            .build()?;

        let response = client
            .get(url)
            .send()
            .await
            .context("Failed to send HTTP request")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "HTTP request failed: {}",
                response.status()
            ));
        }

        let html = response.text().await?;

        // Convert HTML to plain text (Markdown-like)
        // width: 80 characters
        let text = html2text::from_read(html.as_bytes(), 80);

        Ok(text)
    }
}

async fn validate_outbound_url(raw_url: &str) -> Result<Url> {
    let url = Url::parse(raw_url).context("Invalid URL")?;
    match url.scheme() {
        "http" | "https" => {}
        scheme => return Err(anyhow::anyhow!("Unsupported URL scheme: {}", scheme)),
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err(anyhow::anyhow!("URL credentials are not allowed"));
    }

    let host = url.host().context("URL must include a host")?;
    match host {
        Host::Ipv4(ip) => ensure_public_ip(IpAddr::V4(ip))?,
        Host::Ipv6(ip) => ensure_public_ip(IpAddr::V6(ip))?,
        Host::Domain(host) => {
            if is_forbidden_hostname(host) {
                return Err(anyhow::anyhow!("Blocked private or local host: {}", host));
            }

            let port = url
                .port_or_known_default()
                .context("URL must include a valid port")?;
            let addrs = lookup_host((host, port))
                .await
                .with_context(|| format!("Failed to resolve host: {}", host))?;

            let mut resolved = false;
            for addr in addrs {
                resolved = true;
                ensure_public_ip(addr.ip()).with_context(|| {
                    format!("Blocked private or local resolved address for {}", host)
                })?;
            }

            if !resolved {
                return Err(anyhow::anyhow!(
                    "Host did not resolve to any address: {}",
                    host
                ));
            }
        }
    }

    Ok(url)
}

fn is_forbidden_hostname(host: &str) -> bool {
    let normalized = host.trim_end_matches('.').to_ascii_lowercase();
    normalized == "localhost" || normalized.ends_with(".localhost")
}

fn ensure_public_ip(ip: IpAddr) -> Result<()> {
    if is_forbidden_ip(ip) {
        return Err(anyhow::anyhow!("Blocked private or local address: {}", ip));
    }
    Ok(())
}

fn is_forbidden_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_forbidden_ipv4(ip),
        IpAddr::V6(ip) => is_forbidden_ipv6(ip),
    }
}

fn is_forbidden_ipv4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || (octets[0] == 100 && (octets[1] & 0b1100_0000) == 0b0100_0000)
}

fn is_forbidden_ipv6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (segments[0] & 0xfe00) == 0xfc00
        || (segments[0] & 0xffc0) == 0xfe80
        || ip.to_ipv4_mapped().map(is_forbidden_ipv4).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn blocks_localhost_hostname() {
        let err = validate_outbound_url("http://localhost:8080")
            .await
            .expect_err("localhost should be blocked");
        assert!(err.to_string().contains("Blocked private or local host"));
    }

    #[tokio::test]
    async fn blocks_private_ipv4_literal() {
        let err = validate_outbound_url("http://192.168.1.10/status")
            .await
            .expect_err("private IPv4 literals should be blocked");
        assert!(err.to_string().contains("Blocked private or local address"));
    }

    #[tokio::test]
    async fn blocks_ipv6_loopback_literal() {
        let err = validate_outbound_url("http://[::1]/")
            .await
            .expect_err("IPv6 loopback should be blocked");
        assert!(err.to_string().contains("Blocked private or local address"));
    }

    #[tokio::test]
    async fn rejects_non_http_schemes() {
        let err = validate_outbound_url("file:///etc/passwd")
            .await
            .expect_err("non-HTTP schemes should be blocked");
        assert!(err.to_string().contains("Unsupported URL scheme"));
    }

    #[test]
    fn public_dns_ipv4_is_allowed_by_ip_filter() {
        assert!(!is_forbidden_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
    }
}
