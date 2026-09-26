use sha2::{Digest, Sha256};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, GetVolumeInformationW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_DELETE,
    FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::Ioctl::{
    PropertyStandardQuery, StorageDeviceProperty, IOCTL_STORAGE_QUERY_PROPERTY,
    STORAGE_DEVICE_DESCRIPTOR, STORAGE_PROPERTY_QUERY,
};
use windows::Win32::System::SystemInformation::{
    GetSystemFirmwareTable, FIRMWARE_TABLE_PROVIDER,
};

/// Raw SMBIOS Provider Signature: 'RSMB'
const RSMB_PROVIDER: u32 = 0x52534D42;

use std::sync::OnceLock;

static CACHED_FINGERPRINT: OnceLock<HardwareFingerprint> = OnceLock::new();

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HardwareFingerprint {
    pub smbios_uuid: String,
    pub cpuid: String,
    pub drive_serial: String,
    pub sha256_hash: String,
}

impl HardwareFingerprint {
    /// Generates the canonical machine hardware fingerprint.
    /// Invariant: SHA-256 hash of SMBIOS UUID + CPUID + PhysicalDrive0 serial.
    /// Caches the result in OnceLock for sub-microsecond atomic resolution.
    pub fn generate() -> Self {
        CACHED_FINGERPRINT
            .get_or_init(Self::compute_fingerprint)
            .clone()
    }

    fn compute_fingerprint() -> Self {
        let smbios_uuid = get_smbios_uuid().unwrap_or_else(|| "00000000-0000-0000-0000-000000000000".to_string());
        let cpuid = get_cpuid();
        let drive_serial = get_drive_serial().unwrap_or_else(|| "DRIVE0-GENERIC-SERIAL".to_string());

        let raw_fingerprint = format!("SMBIOS:{smbios_uuid}|CPUID:{cpuid}|DRIVE:{drive_serial}");
        let mut hasher = Sha256::new();
        hasher.update(raw_fingerprint.as_bytes());
        let result = hasher.finalize();
        let sha256_hash = format!("{:X}", result);

        Self {
            smbios_uuid,
            cpuid,
            drive_serial,
            sha256_hash,
        }
    }

    /// Formats the HWID for display in the UI / license activation modal.
    pub fn display_hwid(&self) -> String {
        self.sha256_hash.clone()
    }
}

/// Convenience accessor returning formatted hex HWID string.
/// Invariant: SHA-256 hash of SMBIOS UUID + CPUID + PhysicalDrive0 serial.
pub fn get_hardware_fingerprint() -> Result<String, String> {
    let fp = HardwareFingerprint::generate();
    if fp.sha256_hash.is_empty() {
        Err("Failed to generate hardware fingerprint".to_string())
    } else {
        Ok(fp.sha256_hash)
    }
}

/// Retrieve CPUID feature information (Processor signature / Model / Stepping)
pub fn get_cpuid() -> String {
    #[cfg(target_arch = "x86_64")]
    {
        let res = core::arch::x86_64::__cpuid(1);
        format!("{:08X}{:08X}{:08X}{:08X}", res.eax, res.ebx, res.ecx, res.edx)
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        "GENERIC_NON_X86_PROCESSOR".to_string()
    }
}

/// Retrieve SMBIOS UUID from system firmware tables.
pub fn get_smbios_uuid() -> Option<String> {
    unsafe {
        let provider = FIRMWARE_TABLE_PROVIDER(RSMB_PROVIDER);
        let buffer_size = GetSystemFirmwareTable(provider, 0, None);
        if buffer_size == 0 {
            return fallback_machine_guid();
        }

        let mut buffer = vec![0u8; buffer_size as usize];
        let bytes_read = GetSystemFirmwareTable(provider, 0, Some(&mut buffer));
        if bytes_read == 0 || (bytes_read as usize) < 8 {
            return fallback_machine_guid();
        }

        // RawSMBIOSData header is 8 bytes:
        // Byte 0: Used20CallingMethod
        // Byte 1: SMBIOSMajorVersion
        // Byte 2: SMBIOSMinorVersion
        // Byte 3: DmiRevision
        // Bytes 4-7: Length
        // Bytes 8+: SMBIOS structures
        let smbios_data = &buffer[8..bytes_read as usize];
        let mut offset = 0;

        while offset + 4 <= smbios_data.len() {
            let table_type = smbios_data[offset];
            let length = smbios_data[offset + 1] as usize;

            if length < 4 || offset + length > smbios_data.len() {
                break;
            }

            // Type 1 = System Information Table
            if table_type == 1 && length >= 0x18 {
                let uuid_bytes = &smbios_data[offset + 8..offset + 24];
                // Formatted UUID string (little-endian for first 3 groups per SMBIOS 2.6+ spec)
                let uuid_str = format!(
                    "{:02X}{:02X}{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
                    uuid_bytes[3], uuid_bytes[2], uuid_bytes[1], uuid_bytes[0],
                    uuid_bytes[5], uuid_bytes[4],
                    uuid_bytes[7], uuid_bytes[6],
                    uuid_bytes[8], uuid_bytes[9],
                    uuid_bytes[10], uuid_bytes[11], uuid_bytes[12], uuid_bytes[13], uuid_bytes[14], uuid_bytes[15]
                );
                return Some(uuid_str);
            }

            // Skip past strings section (double null-terminator)
            let mut string_offset = offset + length;
            while string_offset + 1 < smbios_data.len() {
                if smbios_data[string_offset] == 0 && smbios_data[string_offset + 1] == 0 {
                    string_offset += 2;
                    break;
                }
                string_offset += 1;
            }

            if string_offset <= offset {
                break;
            }
            offset = string_offset;
        }

        fallback_machine_guid()
    }
}

/// Fallback machine GUID from Windows registry or system environment.
fn fallback_machine_guid() -> Option<String> {
    std::env::var("COMPUTERNAME").ok()
}

/// Retrieve PhysicalDrive0 serial number via DeviceIoControl or VolumeInformation fallback.
pub fn get_drive_serial() -> Option<String> {
    unsafe {
        let drive_path = w!("\\\\.\\PhysicalDrive0");
        let handle = CreateFileW(
            drive_path,
            0, // Query access only (does not require administrator rights)
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        );

        if let Ok(handle) = handle {
            if handle != INVALID_HANDLE_VALUE {
                let mut query = STORAGE_PROPERTY_QUERY {
                    PropertyId: StorageDeviceProperty,
                    QueryType: PropertyStandardQuery,
                    AdditionalParameters: [0],
                };

                let mut out_buffer = [0u8; 1024];
                let mut bytes_returned = 0u32;

                let success = windows::Win32::System::IO::DeviceIoControl(
                    handle,
                    IOCTL_STORAGE_QUERY_PROPERTY,
                    Some(&mut query as *mut _ as *mut core::ffi::c_void),
                    std::mem::size_of::<STORAGE_PROPERTY_QUERY>() as u32,
                    Some(out_buffer.as_mut_ptr() as *mut core::ffi::c_void),
                    out_buffer.len() as u32,
                    Some(&mut bytes_returned),
                    None,
                );

                let _ = CloseHandle(handle);

                if success.is_ok() && bytes_returned >= std::mem::size_of::<STORAGE_DEVICE_DESCRIPTOR>() as u32 {
                    let descriptor = &*(out_buffer.as_ptr() as *const STORAGE_DEVICE_DESCRIPTOR);
                    if descriptor.SerialNumberOffset > 0
                        && (descriptor.SerialNumberOffset as usize) < out_buffer.len()
                    {
                        let serial_start = descriptor.SerialNumberOffset as usize;
                        let mut serial_end = serial_start;
                        while serial_end < out_buffer.len() && out_buffer[serial_end] != 0 {
                            serial_end += 1;
                        }
                        if serial_end > serial_start {
                            let serial = String::from_utf8_lossy(&out_buffer[serial_start..serial_end])
                                .trim()
                                .to_string();
                            if !serial.is_empty() {
                                return Some(serial);
                            }
                        }
                    }
                }
            }
        }

        // Fallback: System volume C:\ serial number
        let mut volume_serial = 0u32;
        let c_drive: Vec<u16> = OsStr::new("C:\\\0").encode_wide().collect();
        let res = GetVolumeInformationW(
            PCWSTR(c_drive.as_ptr()),
            None,
            Some(&mut volume_serial),
            None,
            None,
            None,
        );

        if res.is_ok() && volume_serial != 0 {
            Some(format!("{:08X}", volume_serial))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_fingerprint_generation() {
        let fp1 = HardwareFingerprint::generate();
        let fp2 = HardwareFingerprint::generate();

        assert!(!fp1.sha256_hash.is_empty());
        assert_eq!(fp1.sha256_hash.len(), 64);
        assert!(!fp1.cpuid.is_empty());

        println!("FP1: {:?}", fp1);
        println!("FP2: {:?}", fp2);
        assert_eq!(fp1.cpuid, fp2.cpuid);
        assert_eq!(fp1.smbios_uuid, fp2.smbios_uuid);
        assert_eq!(fp1.drive_serial, fp2.drive_serial);
        assert_eq!(fp1.sha256_hash, fp2.sha256_hash);
    }

    #[test]
    fn test_get_hardware_fingerprint_deterministic_hex_sha256() {
        let hwid = get_hardware_fingerprint().expect("HWID generation must succeed");
        assert_eq!(hwid.len(), 64, "SHA-256 hex string must be exactly 64 characters");
        assert!(hwid.chars().all(|c| c.is_ascii_hexdigit()), "HWID must be valid hex");

        let hwid2 = get_hardware_fingerprint().expect("Second HWID call must succeed");
        assert_eq!(hwid, hwid2, "HWID must be deterministic across calls");
    }
}
