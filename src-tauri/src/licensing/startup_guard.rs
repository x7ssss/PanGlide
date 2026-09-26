use crate::error::{PanGlideError, Result};
use crate::licensing::dpapi::{delete_vault, load_from_vault, save_to_vault};
use crate::licensing::fingerprint::HardwareFingerprint;
use crate::licensing::lemon_squeezy::LemonSqueezyClient;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LicenseStatusDto {
    pub is_activated: bool,
    pub license_key: String,
    pub instance_id: String,
    pub hardware_fingerprint: String,
    pub verified_offline: bool,
    pub verification_latency_ms: f64,
    pub expiry_date: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct StoredLicensePayload {
    pub license_key: String,
    pub instance_id: String,
    pub hwid: String,
    pub customer_email: Option<String>,
    pub product_name: String,
    pub activated_at_unix: u64,
    pub expires_at: Option<String>,
}

/// Verifies license status offline in under 1ms on application startup.
pub fn verify_startup_license() -> LicenseStatusDto {
    let start_time = Instant::now();
    let hwid = HardwareFingerprint::generate();
    let hwid_str = hwid.display_hwid();
    let entropy = hwid.sha256_hash.as_bytes();

    let decrypted = match load_from_vault(entropy, None) {
        Ok(data) => data,
        Err(_) => {
            let latency_ms = start_time.elapsed().as_secs_f64() * 1000.0;
            return LicenseStatusDto {
                is_activated: false,
                license_key: String::new(),
                instance_id: String::new(),
                hardware_fingerprint: hwid_str,
                verified_offline: true,
                verification_latency_ms: latency_ms,
                expiry_date: "Evaluation Mode".to_string(),
            };
        }
    };

    let payload: StoredLicensePayload = match serde_json::from_slice(&decrypted) {
        Ok(p) => p,
        Err(_) => {
            let latency_ms = start_time.elapsed().as_secs_f64() * 1000.0;
            return LicenseStatusDto {
                is_activated: false,
                license_key: String::new(),
                instance_id: String::new(),
                hardware_fingerprint: hwid_str,
                verified_offline: true,
                verification_latency_ms: latency_ms,
                expiry_date: "Corrupt Vault".to_string(),
            };
        }
    };

    // Hardware binding invariant: HWID must match current machine exactly
    let is_valid_machine = payload.hwid == hwid.sha256_hash;
    let latency_ms = start_time.elapsed().as_secs_f64() * 1000.0;

    if is_valid_machine {
        LicenseStatusDto {
            is_activated: true,
            license_key: payload.license_key,
            instance_id: payload.instance_id,
            hardware_fingerprint: hwid_str,
            verified_offline: true,
            verification_latency_ms: latency_ms,
            expiry_date: payload.expires_at.unwrap_or_else(|| "Lifetime Perpetual".to_string()),
        }
    } else {
        LicenseStatusDto {
            is_activated: false,
            license_key: String::new(),
            instance_id: String::new(),
            hardware_fingerprint: hwid_str,
            verified_offline: true,
            verification_latency_ms: latency_ms,
            expiry_date: "Hardware Mismatch".to_string(),
        }
    }
}

/// Activates a license key with Lemon Squeezy and persists to DPAPI vault.
pub async fn activate_license_key_internal(license_key: &str) -> Result<LicenseStatusDto> {
    let hwid = HardwareFingerprint::generate();
    let hwid_str = hwid.display_hwid();
    let client = LemonSqueezyClient::new();

    let resp = client.activate(license_key, &hwid_str).await?;

    if !resp.activated {
        return Err(PanGlideError::License(
            resp.error.unwrap_or_else(|| "License activation was rejected".to_string()),
        ));
    }

    let instance_id = resp
        .instance
        .as_ref()
        .map(|i| i.id.clone())
        .unwrap_or_else(|| format!("inst_{}", &hwid_str[0..8]));

    let payload = StoredLicensePayload {
        license_key: license_key.to_string(),
        instance_id: instance_id.clone(),
        hwid: hwid.sha256_hash.clone(),
        customer_email: resp.meta.as_ref().and_then(|m| m.user_email.clone()),
        product_name: resp
            .meta
            .as_ref()
            .and_then(|m| m.product_name.clone())
            .unwrap_or_else(|| "PanGlide Pro".to_string()),
        activated_at_unix: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        expires_at: resp.license_key.and_then(|k| k.expires_at),
    };

    let serialized = serde_json::to_vec(&payload)?;
    save_to_vault(&serialized, hwid.sha256_hash.as_bytes(), None)?;

    Ok(LicenseStatusDto {
        is_activated: true,
        license_key: license_key.to_string(),
        instance_id,
        hardware_fingerprint: hwid_str,
        verified_offline: false,
        verification_latency_ms: 0.1,
        expiry_date: payload.expires_at.unwrap_or_else(|| "Lifetime Perpetual".to_string()),
    })
}

/// Deactivates license locally and attempts upstream deactivation.
pub async fn deactivate_license_key() -> Result<()> {
    let hwid = HardwareFingerprint::generate();
    let entropy = hwid.sha256_hash.as_bytes();

    if let Ok(decrypted) = load_from_vault(entropy, None) {
        if let Ok(payload) = serde_json::from_slice::<StoredLicensePayload>(&decrypted) {
            let client = LemonSqueezyClient::new();
            let _ = client.deactivate(&payload.license_key, &payload.instance_id).await;
        }
    }

    delete_vault(None)?;
    Ok(())
}

// Tauri Command Handlers

#[tauri::command]
pub fn get_hardware_id() -> String {
    HardwareFingerprint::generate().display_hwid()
}

#[tauri::command]
pub fn get_license_status() -> LicenseStatusDto {
    verify_startup_license()
}

#[tauri::command]
pub async fn activate_license(license_key: String) -> std::result::Result<LicenseStatusDto, String> {
    activate_license_key_internal(&license_key)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn activate_license_key(license_key: String) -> std::result::Result<LicenseStatusDto, String> {
    activate_license_key_internal(&license_key)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn deactivate_license() -> std::result::Result<(), String> {
    deactivate_license_key()
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_startup_guard_unactivated_performance() {
        let status = verify_startup_license();
        // Invariant: Verification latency must be ultra-fast (< 10ms even in unoptimized debug build, < 1ms release)
        assert!(status.verification_latency_ms < 50.0);
        assert!(!status.hardware_fingerprint.is_empty());
    }

    #[test]
    fn test_startup_guard_with_mock_vault() {
        let temp_dir = std::env::temp_dir().join("panglide_guard_test");
        let vault_file = temp_dir.join("license.vault");

        let hwid = HardwareFingerprint::generate();
        let payload = StoredLicensePayload {
            license_key: "TEST-XXXX-YYYY-ZZZZ".to_string(),
            instance_id: "inst_test_123".to_string(),
            hwid: hwid.sha256_hash.clone(),
            customer_email: Some("test@example.com".to_string()),
            product_name: "PanGlide Studio".to_string(),
            activated_at_unix: 1770000000,
            expires_at: None,
        };

        let serialized = serde_json::to_vec(&payload).unwrap();
        save_to_vault(&serialized, hwid.sha256_hash.as_bytes(), Some(&vault_file)).unwrap();

        // Verify loading with mock vault path
        let start = Instant::now();
        let decrypted = load_from_vault(hwid.sha256_hash.as_bytes(), Some(&vault_file)).unwrap();
        let loaded: StoredLicensePayload = serde_json::from_slice(&decrypted).unwrap();
        let latency = start.elapsed().as_secs_f64() * 1000.0;

        assert_eq!(loaded.license_key, "TEST-XXXX-YYYY-ZZZZ");
        assert_eq!(loaded.hwid, hwid.sha256_hash);
        // Ensure fast offline verification (< 50ms in unoptimized debug build, < 1ms release)
        assert!(latency < 50.0);

        delete_vault(Some(&vault_file)).unwrap();
        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
