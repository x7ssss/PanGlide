use crate::error::{PanGlideError, Result};
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const DEFAULT_BASE_URL: &str = "https://api.lemonsqueezy.com/v1/licenses";

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LemonSqueezyActivateResponse {
    pub activated: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub license_key: Option<LemonLicenseKeyInfo>,
    #[serde(default)]
    pub instance: Option<LemonInstanceInfo>,
    #[serde(default)]
    pub meta: Option<LemonMetaInfo>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LemonLicenseKeyInfo {
    pub id: Option<serde_json::Value>,
    pub status: String,
    pub key: String,
    pub activation_limit: Option<u32>,
    pub activation_usage: Option<u32>,
    pub created_at: Option<String>,
    pub expires_at: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LemonInstanceInfo {
    pub id: String,
    pub name: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LemonMetaInfo {
    pub store_id: Option<u64>,
    pub order_id: Option<u64>,
    pub user_name: Option<String>,
    pub user_email: Option<String>,
    pub product_name: Option<String>,
}

#[derive(Clone)]
pub struct LemonSqueezyClient {
    client: reqwest::Client,
    base_url: String,
}

impl LemonSqueezyClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            base_url: DEFAULT_BASE_URL.to_string(),
        }
    }

    pub fn with_base_url(base_url: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
            base_url: base_url.to_string(),
        }
    }

    /// Activates a license key with Lemon Squeezy API.
    /// Invariant: instance_name is set to the machine HWID.
    pub async fn activate(&self, license_key: &str, instance_name: &str) -> Result<LemonSqueezyActivateResponse> {
        let url = format!("{}/activate", self.base_url);

        let mut params = HashMap::new();
        params.insert("license_key", license_key);
        params.insert("instance_name", instance_name);

        let response = self
            .client
            .post(&url)
            .header(ACCEPT, "application/json")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await
            .map_err(|e| PanGlideError::License(format!("Lemon Squeezy network error: {e}")))?;

        let activate_resp = response
            .json::<LemonSqueezyActivateResponse>()
            .await
            .map_err(|e| PanGlideError::License(format!("Failed to parse Lemon Squeezy response: {e}")))?;

        Ok(activate_resp)
    }

    /// Validates an existing license key instance with Lemon Squeezy.
    pub async fn validate(&self, license_key: &str, instance_id: &str) -> Result<LemonSqueezyActivateResponse> {
        let url = format!("{}/validate", self.base_url);

        let mut params = HashMap::new();
        params.insert("license_key", license_key);
        params.insert("instance_id", instance_id);

        let response = self
            .client
            .post(&url)
            .header(ACCEPT, "application/json")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await
            .map_err(|e| PanGlideError::License(format!("Lemon Squeezy validation network error: {e}")))?;

        let validate_resp = response
            .json::<LemonSqueezyActivateResponse>()
            .await
            .map_err(|e| PanGlideError::License(format!("Failed to parse Lemon Squeezy response: {e}")))?;

        Ok(validate_resp)
    }

    /// Deactivates a license instance from Lemon Squeezy.
    pub async fn deactivate(&self, license_key: &str, instance_id: &str) -> Result<bool> {
        let url = format!("{}/deactivate", self.base_url);

        let mut params = HashMap::new();
        params.insert("license_key", license_key);
        params.insert("instance_id", instance_id);

        let response = self
            .client
            .post(&url)
            .header(ACCEPT, "application/json")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .form(&params)
            .send()
            .await
            .map_err(|e| PanGlideError::License(format!("Lemon Squeezy deactivation network error: {e}")))?;

        #[derive(Deserialize)]
        struct DeactivateResp {
            deactivated: bool,
        }

        let resp = response
            .json::<DeactivateResp>()
            .await
            .map_err(|e| PanGlideError::License(format!("Failed to parse deactivation response: {e}")))?;

        Ok(resp.deactivated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_lemon_squeezy_response() {
        let json_data = r#"{
            "activated": true,
            "error": null,
            "license_key": {
                "id": 99482,
                "status": "active",
                "key": "PAN-GLIDE-PRO-2026-KEY",
                "activation_limit": 3,
                "activation_usage": 1,
                "created_at": "2026-09-26T12:00:00.000000Z",
                "expires_at": null
            },
            "instance": {
                "id": "inst_918273645",
                "name": "HWID-TEST-MACHINE",
                "created_at": "2026-09-26T12:05:00.000000Z"
            },
            "meta": {
                "store_id": 1042,
                "order_id": 88412,
                "user_name": "PanGlide Creator",
                "user_email": "creator@example.com",
                "product_name": "PanGlide Pro Lifetime"
            }
        }"#;

        let parsed: LemonSqueezyActivateResponse = serde_json::from_str(json_data).unwrap();
        assert!(parsed.activated);
        assert_eq!(parsed.license_key.unwrap().key, "PAN-GLIDE-PRO-2026-KEY");
        assert_eq!(parsed.instance.unwrap().id, "inst_918273645");
        assert_eq!(parsed.meta.unwrap().product_name.unwrap(), "PanGlide Pro Lifetime");
    }
}
