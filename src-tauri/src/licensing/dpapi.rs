use crate::error::{PanGlideError, Result};
use std::fs;
use std::path::{Path, PathBuf};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{LocalFree, HLOCAL};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

/// Encrypts plaintext bytes using Windows Data Protection API (DPAPI) tied to current user and hardware entropy.
pub fn dpapi_encrypt(plaintext: &[u8], optional_entropy: Option<&[u8]>) -> Result<Vec<u8>> {
    let mut in_data = plaintext.to_vec();
    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: in_data.len() as u32,
        pbData: in_data.as_mut_ptr(),
    };

    let mut out_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    let mut entropy_data = optional_entropy.map(|e| e.to_vec());
    let entropy_blob = entropy_data.as_mut().map(|e| CRYPT_INTEGER_BLOB {
        cbData: e.len() as u32,
        pbData: e.as_mut_ptr(),
    });
    let p_entropy = entropy_blob.as_ref().map(|b| b as *const CRYPT_INTEGER_BLOB);

    unsafe {
        let res = CryptProtectData(
            &in_blob,
            PCWSTR::null(),
            p_entropy,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        );

        if res.is_err() || out_blob.pbData.is_null() {
            return Err(PanGlideError::License(format!(
                "DPAPI CryptProtectData failed: {:?}",
                res.err()
            )));
        }

        let encrypted_bytes =
            std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();

        let _ = LocalFree(HLOCAL(out_blob.pbData as _));

        Ok(encrypted_bytes)
    }
}

/// Decrypts ciphertext bytes using Windows DPAPI and matching entropy.
pub fn dpapi_decrypt(ciphertext: &[u8], optional_entropy: Option<&[u8]>) -> Result<Vec<u8>> {
    let mut in_data = ciphertext.to_vec();
    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: in_data.len() as u32,
        pbData: in_data.as_mut_ptr(),
    };

    let mut out_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    };

    let mut entropy_data = optional_entropy.map(|e| e.to_vec());
    let entropy_blob = entropy_data.as_mut().map(|e| CRYPT_INTEGER_BLOB {
        cbData: e.len() as u32,
        pbData: e.as_mut_ptr(),
    });
    let p_entropy = entropy_blob.as_ref().map(|b| b as *const CRYPT_INTEGER_BLOB);

    unsafe {
        let res = CryptUnprotectData(
            &in_blob,
            None,
            p_entropy,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        );

        if res.is_err() || out_blob.pbData.is_null() {
            return Err(PanGlideError::License(format!(
                "DPAPI CryptUnprotectData failed: {:?}",
                res.err()
            )));
        }

        let decrypted_bytes =
            std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();

        let _ = LocalFree(HLOCAL(out_blob.pbData as _));

        Ok(decrypted_bytes)
    }
}

/// Returns the platform vault file path on Windows (%LOCALAPPDATA%\PanGlide\panglide.lic).
pub fn get_vault_path() -> PathBuf {
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let lic_path = PathBuf::from(&local_app_data).join("PanGlide").join("panglide.lic");
        let legacy_vault = PathBuf::from(&local_app_data).join("PanGlide").join("license.vault");
        if !lic_path.exists() && legacy_vault.exists() {
            legacy_vault
        } else {
            lic_path
        }
    } else if let Ok(app_data) = std::env::var("APPDATA") {
        PathBuf::from(app_data).join("PanGlide").join("panglide.lic")
    } else {
        PathBuf::from(".panglide").join("panglide.lic")
    }
}

/// Saves encrypted license data to the DPAPI vault.
pub fn save_to_vault(data: &[u8], entropy: &[u8], vault_path: Option<&Path>) -> Result<()> {
    let path = vault_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(get_vault_path);

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let encrypted = dpapi_encrypt(data, Some(entropy))?;
    fs::write(&path, encrypted)?;
    Ok(())
}

/// Loads and decrypts license data from the DPAPI vault.
pub fn load_from_vault(entropy: &[u8], vault_path: Option<&Path>) -> Result<Vec<u8>> {
    let path = vault_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(get_vault_path);

    if !path.exists() {
        return Err(PanGlideError::License("Vault file does not exist".to_string()));
    }

    let encrypted = fs::read(&path)?;
    let decrypted = dpapi_decrypt(&encrypted, Some(entropy))?;
    Ok(decrypted)
}

/// Deletes the local vault file upon deactivation.
pub fn delete_vault(vault_path: Option<&Path>) -> Result<()> {
    let path = vault_path
        .map(|p| p.to_path_buf())
        .unwrap_or_else(get_vault_path);

    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpapi_roundtrip_with_hwid_entropy() {
        let plaintext = b"PANGLIDE_SECRET_LICENSE_DATA_2026";
        let hwid_entropy = b"4F8E2A1B7C3D5E6F8091A2B3C4D5E6F7";

        let encrypted = dpapi_encrypt(plaintext, Some(hwid_entropy)).expect("DPAPI encrypt failed");
        assert_ne!(encrypted, plaintext);

        let decrypted = dpapi_decrypt(&encrypted, Some(hwid_entropy)).expect("DPAPI decrypt failed");
        assert_eq!(decrypted, plaintext);

        // Mismatched entropy must fail decryption
        let bad_entropy = b"WRONG_MACHINE_ENTROPY_0000000000";
        let fail_result = dpapi_decrypt(&encrypted, Some(bad_entropy));
        assert!(fail_result.is_err());
    }

    #[test]
    fn test_dpapi_vault_file_persistence() {
        let temp_dir = std::env::temp_dir().join("panglide_test_vault");
        let vault_file = temp_dir.join("test_license.vault");

        let payload = b"{\"activated\":true,\"product\":\"PanGlide Pro\"}";
        let entropy = b"MACHINE_HWID_SAMPLE";

        save_to_vault(payload, entropy, Some(&vault_file)).expect("Save failed");
        assert!(vault_file.exists());

        let loaded = load_from_vault(entropy, Some(&vault_file)).expect("Load failed");
        assert_eq!(loaded, payload);

        delete_vault(Some(&vault_file)).expect("Delete failed");
        assert!(!vault_file.exists());

        let _ = fs::remove_dir_all(temp_dir);
    }
}
