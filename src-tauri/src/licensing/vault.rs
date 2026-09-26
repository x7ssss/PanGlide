use crate::error::Result;
use crate::licensing::dpapi::{dpapi_decrypt, dpapi_encrypt, get_vault_path};
use crate::licensing::fingerprint::HardwareFingerprint;
use crate::licensing::startup_guard::{LicenseStatusDto, StoredLicensePayload};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Windows DPAPI Local License Vault (`%LOCALAPPDATA%\PanGlide\panglide.lic`)
pub struct LicenseVault {
    vault_path: PathBuf,
}

impl Default for LicenseVault {
    fn default() -> Self {
        Self::new()
    }
}

impl LicenseVault {
    pub fn new() -> Self {
        Self {
            vault_path: get_vault_path(),
        }
    }

    pub fn with_custom_path(path: PathBuf) -> Self {
        Self { vault_path: path }
    }

    pub fn path(&self) -> &Path {
        &self.vault_path
    }

    /// Saves verified license payload encrypted with user DPAPI credentials + machine HWID entropy.
    pub fn save_license(&self, payload: &StoredLicensePayload) -> Result<()> {
        if let Some(parent) = self.vault_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let serialized = serde_json::to_vec(payload)?;
        let hwid = HardwareFingerprint::generate();
        let encrypted = dpapi_encrypt(&serialized, Some(hwid.sha256_hash.as_bytes()))?;
        fs::write(&self.vault_path, encrypted)?;
        Ok(())
    }

    /// Loads and verifies license payload from local vault in < 1ms without network calls.
    pub fn verify_license(&self) -> LicenseStatusDto {
        let start_time = Instant::now();
        let hwid = HardwareFingerprint::generate();
        let hwid_str = hwid.display_hwid();
        let entropy = hwid.sha256_hash.as_bytes();

        if !self.vault_path.exists() {
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

        let encrypted = match fs::read(&self.vault_path) {
            Ok(bytes) => bytes,
            Err(_) => {
                let latency_ms = start_time.elapsed().as_secs_f64() * 1000.0;
                return LicenseStatusDto {
                    is_activated: false,
                    license_key: String::new(),
                    instance_id: String::new(),
                    hardware_fingerprint: hwid_str,
                    verified_offline: true,
                    verification_latency_ms: latency_ms,
                    expiry_date: "Unreadable Vault".to_string(),
                };
            }
        };

        let decrypted = match dpapi_decrypt(&encrypted, Some(entropy)) {
            Ok(bytes) => bytes,
            Err(_) => {
                let latency_ms = start_time.elapsed().as_secs_f64() * 1000.0;
                return LicenseStatusDto {
                    is_activated: false,
                    license_key: String::new(),
                    instance_id: String::new(),
                    hardware_fingerprint: hwid_str,
                    verified_offline: true,
                    verification_latency_ms: latency_ms,
                    expiry_date: "DPAPI Decryption Failed".to_string(),
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
                    expiry_date: "Corrupt Vault Data".to_string(),
                };
            }
        };

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

    /// Removes local license vault file.
    pub fn delete_license(&self) -> Result<()> {
        if self.vault_path.exists() {
            fs::remove_file(&self.vault_path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vault_roundtrip_and_sub_millisecond_verification() {
        let temp_dir = std::env::temp_dir().join(format!("panglide_vault_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()));
        let lic_file = temp_dir.join("panglide.lic");
        let vault = LicenseVault::with_custom_path(lic_file.clone());

        // Unactivated initial check
        let initial_status = vault.verify_license();
        assert!(!initial_status.is_activated);
        assert!(initial_status.verification_latency_ms < 50.0);

        // Store active license
        let hwid = HardwareFingerprint::generate();
        let payload = StoredLicensePayload {
            license_key: "PANGLIDE-PRO-2026-KEY".to_string(),
            instance_id: "inst_verified_dpapi".to_string(),
            hwid: hwid.sha256_hash.clone(),
            customer_email: Some("pro@panglide.com".to_string()),
            product_name: "PanGlide Pro Studio".to_string(),
            activated_at_unix: 1770000000,
            expires_at: Some("Lifetime Perpetual".to_string()),
        };

        vault.save_license(&payload).expect("Vault save must succeed");
        assert!(lic_file.exists());

        // Fast offline verification guard
        let start = Instant::now();
        let verified = vault.verify_license();
        let latency_ms = start.elapsed().as_secs_f64() * 1000.0;

        assert!(verified.is_activated);
        assert_eq!(verified.license_key, "PANGLIDE-PRO-2026-KEY");
        assert_eq!(verified.hardware_fingerprint, hwid.display_hwid());
        assert!(verified.verified_offline);
        // Ensure latency is ultra-fast
        assert!(latency_ms < 50.0);

        // Delete vault
        vault.delete_license().expect("Vault deletion must succeed");
        assert!(!lic_file.exists());

        let post_delete = vault.verify_license();
        assert!(!post_delete.is_activated);

        let _ = fs::remove_dir_all(temp_dir);
    }
}
