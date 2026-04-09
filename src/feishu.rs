use reqwest::Client;
use serde_json::json;
use tracing::warn;

pub struct FeishuNotifier {
    client: Client,
    webhook_url: String,
    proxy: Option<String>,
}

impl FeishuNotifier {
    pub fn new(webhook_url: String, proxy: Option<String>) -> Self {
        let client = if let Some(ref p) = proxy {
            match reqwest::Proxy::all(p) {
                Ok(proxy_obj) => reqwest::Client::builder()
                    .proxy(proxy_obj)
                    .build()
                    .unwrap_or_default(),
                Err(_) => reqwest::Client::new(),
            }
        } else {
            reqwest::Client::new()
        };
        Self { client, webhook_url, proxy }
    }

    pub async fn send(&self, text: &str) {
        let body = json!({
            "msg_type": "text",
            "content": { "text": text }
        });
        match self
            .client
            .post(&self.webhook_url)
            .json(&body)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {}
            Ok(resp) => warn!("Feishu webhook returned {}", resp.status()),
            Err(e) => warn!("Feishu webhook error: {e}"),
        }
    }
}
