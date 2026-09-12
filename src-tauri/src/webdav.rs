use anyhow::{anyhow, Context};
use reqwest::StatusCode;
use serde::Serialize;
use url::Url;

use crate::core::Result;
use crate::settings::WebDav;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebDavSyncResult {
    pub bytes: u64,
    pub remote_url: String,
}

pub fn remote_object_url(settings: &WebDav) -> Result<String> {
    let mut url = Url::parse(settings.url.trim()).context("WebDAV URL is invalid")?;
    if url.scheme() != "https" {
        return Err(anyhow!("WebDAV URL must use https://").into());
    }

    let file_name = settings.file_name.trim();
    if file_name.is_empty() {
        return Err(anyhow!("WebDAV file name is empty").into());
    }
    if file_name.contains('/') || file_name.contains('\\') {
        return Err(anyhow!("WebDAV file name must not contain a path").into());
    }

    url.path_segments_mut()
        .map_err(|_| anyhow!("WebDAV URL cannot be used as a base URL"))?
        .pop_if_empty()
        .push(file_name);

    Ok(url.into())
}

pub async fn put_bytes(settings: &WebDav, body: Vec<u8>) -> Result<WebDavSyncResult> {
    let remote_url = remote_object_url(settings)?;
    let client = reqwest::Client::builder()
        .build()
        .context("failed to create WebDAV client")?;
    let bytes = body.len() as u64;
    let response = client
        .put(&remote_url)
        .basic_auth(&settings.username, Some(&settings.password))
        .header("Content-Type", "application/octet-stream")
        .body(body)
        .send()
        .await
        .context("WebDAV upload failed")?;
    let status = response.status();
    if !status.is_success() {
        let detail = response.text().await.unwrap_or_default();
        return Err(anyhow!("WebDAV upload failed ({status}): {detail}").into());
    }

    Ok(WebDavSyncResult { bytes, remote_url })
}

pub async fn get_bytes(settings: &WebDav) -> Result<(Vec<u8>, String)> {
    let remote_url = remote_object_url(settings)?;
    let client = reqwest::Client::builder()
        .build()
        .context("failed to create WebDAV client")?;
    let response = client
        .get(&remote_url)
        .basic_auth(&settings.username, Some(&settings.password))
        .send()
        .await
        .context("WebDAV download failed")?;
    let status = response.status();
    if status == StatusCode::NOT_FOUND {
        return Err(anyhow!("No clipboard backup found on the WebDAV server").into());
    }
    if !status.is_success() {
        let detail = response.text().await.unwrap_or_default();
        return Err(anyhow!("WebDAV download failed ({status}): {detail}").into());
    }

    let body = response
        .bytes()
        .await
        .context("failed to read WebDAV response")?
        .to_vec();

    Ok((body, remote_url))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings_with_url(url: &str) -> WebDav {
        WebDav {
            enabled: true,
            url: url.to_owned(),
            username: "me".to_owned(),
            password: "secret".to_owned(),
            file_name: "clipboard.ecopastebak".to_owned(),
        }
    }

    #[test]
    fn joins_base_url_and_file_name() {
        let settings = settings_with_url("https://cloud.example/dav/files/me/");

        assert_eq!(
            remote_object_url(&settings).unwrap(),
            "https://cloud.example/dav/files/me/clipboard.ecopastebak"
        );
    }

    #[test]
    fn rejects_http_before_basic_auth_can_be_sent() {
        let settings = settings_with_url("http://cloud.example/dav/files/me");

        let error = remote_object_url(&settings).unwrap_err();

        assert!(error.to_string().contains("must use https://"));
    }
}
