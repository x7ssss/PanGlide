pub mod client;
pub mod dpapi;
pub mod fingerprint;
pub mod lemon_squeezy;
pub mod startup_guard;
pub mod vault;

pub use fingerprint::{get_hardware_fingerprint, HardwareFingerprint};
pub use startup_guard::{
    activate_license, activate_license_key, deactivate_license, get_hardware_id,
    get_license_status, LicenseStatusDto, StoredLicensePayload,
};
pub use vault::LicenseVault;

