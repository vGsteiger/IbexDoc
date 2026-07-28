#[cfg(target_os = "macos")]
use crate::constants::{DB_KEY_ACCOUNT, FS_KEY_ACCOUNT};
use crate::error::AppError;

#[cfg(target_os = "macos")]
use core_foundation::base::{CFRelease, CFTypeRef, TCFType};
#[cfg(target_os = "macos")]
use core_foundation::boolean::CFBoolean;
#[cfg(target_os = "macos")]
use core_foundation::data::CFData;
#[cfg(target_os = "macos")]
use core_foundation::dictionary::CFDictionary;
#[cfg(target_os = "macos")]
use core_foundation::string::CFString;
#[cfg(target_os = "macos")]
use security_framework::access_control::{ProtectionMode, SecAccessControl};
#[cfg(target_os = "macos")]
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password, AccessControlOptions,
};
#[cfg(target_os = "macos")]
use security_framework_sys::base::errSecItemNotFound;
#[cfg(target_os = "macos")]
use security_framework_sys::item::{
    kSecAttrAccessControl, kSecAttrAccount, kSecAttrService, kSecClass, kSecClassGenericPassword,
    kSecReturnAttributes, kSecValueData,
};
#[cfg(target_os = "macos")]
use security_framework_sys::keychain_item::{SecItemAdd, SecItemCopyMatching, SecItemDelete};

#[cfg(target_os = "macos")]
const MASTER_KEY_ACCESS_CONTROL_VERSION_ACCOUNT: &str = "master-key-access-control-version";
#[cfg(target_os = "macos")]
const MASTER_KEY_ACCESS_CONTROL_VERSION: &[u8] = b"1";

/// Store a master key in macOS Keychain with OS-enforced authentication.
///
/// The item is device-bound and requires either the currently enrolled biometric
/// set or the device passcode for every data read. Changing the enrolled biometric
/// set invalidates the item, in which case recovery is required.
#[cfg(target_os = "macos")]
pub fn store_key(service: &str, account: &str, key: &[u8]) -> Result<(), AppError> {
    // Delete any existing item first using a raw SecItemDelete query so we match
    // items regardless of how they were originally stored.
    let del_query = CFDictionary::<CFString, _>::from_CFType_pairs(&[
        (
            unsafe { CFString::wrap_under_get_rule(kSecClass) },
            unsafe { CFString::wrap_under_get_rule(kSecClassGenericPassword) }.as_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecAttrService) },
            CFString::new(service).as_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecAttrAccount) },
            CFString::new(account).as_CFType(),
        ),
    ]);
    unsafe { SecItemDelete(del_query.as_concrete_TypeRef()) };

    let access_control = SecAccessControl::create_with_protection(
        Some(ProtectionMode::AccessibleWhenPasscodeSetThisDeviceOnly),
        (AccessControlOptions::BIOMETRY_CURRENT_SET
            | AccessControlOptions::OR
            | AccessControlOptions::DEVICE_PASSCODE)
            .bits(),
    )
    .map_err(|e| AppError::Keychain(format!("Failed to create key access control: {}", e)))?;

    let dict = CFDictionary::<CFString, _>::from_CFType_pairs(&[
        (
            unsafe { CFString::wrap_under_get_rule(kSecClass) },
            unsafe { CFString::wrap_under_get_rule(kSecClassGenericPassword) }.as_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecAttrService) },
            CFString::new(service).as_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecAttrAccount) },
            CFString::new(account).as_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecValueData) },
            CFData::from_buffer(key).as_CFType(),
        ),
        (
            unsafe { CFString::wrap_under_get_rule(kSecAttrAccessControl) },
            access_control.as_CFType(),
        ),
    ]);

    let status = unsafe { SecItemAdd(dict.as_concrete_TypeRef(), std::ptr::null_mut()) };
    if status != 0 {
        return Err(AppError::Keychain(format!(
            "Failed to store key (OSStatus {})",
            status
        )));
    }

    Ok(())
}

/// Retrieve a master key from Keychain.
///
/// This function itself does not show Touch ID — biometric authentication is
/// handled by `touch_id::authenticate` in the caller (`unlock_app`) before
/// this function is invoked.
#[cfg(target_os = "macos")]
pub fn retrieve_key(service: &str, account: &str) -> Result<Vec<u8>, AppError> {
    get_generic_password(service, account)
        .map(|p| p.to_vec())
        .map_err(|e| AppError::Keychain(format!("Failed to retrieve key: {}", e)))
}

/// Recreate legacy master-key items with the current OS-enforced protection.
///
/// Versions before the biometric Keychain migration stored master keys without a
/// `SecAccessControl` object. After a successful legacy unlock, this upgrades both
/// items before the application enters the unlocked state. The version marker is
/// metadata only; it contains no key material and must never be used to authorize
/// access to the master keys.
#[cfg(target_os = "macos")]
pub fn migrate_master_keys_to_biometric_protection(
    service: &str,
    db_key: &[u8],
    fs_key: &[u8],
) -> Result<(), AppError> {
    let migration_complete = retrieve_metadata(service, MASTER_KEY_ACCESS_CONTROL_VERSION_ACCOUNT)
        .is_ok_and(|version| version == MASTER_KEY_ACCESS_CONTROL_VERSION);
    if migration_complete {
        return Ok(());
    }

    store_key(service, DB_KEY_ACCOUNT, db_key)?;
    store_key(service, FS_KEY_ACCOUNT, fs_key)?;
    store_metadata(
        service,
        MASTER_KEY_ACCESS_CONTROL_VERSION_ACCOUNT,
        MASTER_KEY_ACCESS_CONTROL_VERSION,
    )
}

/// Delete a key from Keychain.
#[cfg(target_os = "macos")]
pub fn delete_key(service: &str, account: &str) -> Result<(), AppError> {
    delete_generic_password(service, account)
        .map_err(|e| AppError::Keychain(format!("Failed to delete key: {}", e)))
}

/// Store non-sensitive metadata in Keychain **without** biometric protection.
///
/// Uses the standard `set_generic_password` API which stores items with
/// `kSecAttrAccessibleAfterFirstUnlock` accessibility — readable after device boot
/// without Touch ID. Intended for data like recovery attempt counters that must be
/// readable before the user has authenticated.
#[cfg(target_os = "macos")]
pub fn store_metadata(service: &str, account: &str, data: &[u8]) -> Result<(), AppError> {
    set_generic_password(service, account, data)
        .map_err(|e| AppError::Keychain(format!("Failed to store metadata: {}", e)))
}

/// Retrieve non-sensitive metadata from Keychain without triggering Touch ID.
#[cfg(target_os = "macos")]
pub fn retrieve_metadata(service: &str, account: &str) -> Result<Vec<u8>, AppError> {
    get_generic_password(service, account)
        .map(|p| p.to_vec())
        .map_err(|e| AppError::Keychain(format!("Failed to retrieve metadata: {}", e)))
}

/// Delete non-sensitive metadata from Keychain.
#[cfg(target_os = "macos")]
pub fn delete_metadata(service: &str, account: &str) -> Result<(), AppError> {
    delete_generic_password(service, account)
        .map_err(|e| AppError::Keychain(format!("Failed to delete metadata: {}", e)))
}

/// Check if both master keys exist in the Keychain WITHOUT triggering Touch ID.
///
/// Uses a `SecItemCopyMatching` query that requests only item attributes
/// (`kSecReturnAttributes = true`), never the secret data. macOS does not
/// require biometric authentication for attribute-only queries, so this
/// function is safe to call at cold-boot state determination.
#[cfg(target_os = "macos")]
pub fn keys_exist(service: &str) -> Result<bool, AppError> {
    for account in [DB_KEY_ACCOUNT, FS_KEY_ACCOUNT] {
        let query = CFDictionary::<CFString, _>::from_CFType_pairs(&[
            (
                unsafe { CFString::wrap_under_get_rule(kSecClass) },
                unsafe { CFString::wrap_under_get_rule(kSecClassGenericPassword) }.as_CFType(),
            ),
            (
                unsafe { CFString::wrap_under_get_rule(kSecAttrService) },
                CFString::new(service).as_CFType(),
            ),
            (
                unsafe { CFString::wrap_under_get_rule(kSecAttrAccount) },
                CFString::new(account).as_CFType(),
            ),
            (
                unsafe { CFString::wrap_under_get_rule(kSecReturnAttributes) },
                CFBoolean::true_value().as_CFType(),
            ),
        ]);

        let mut result: CFTypeRef = std::ptr::null();
        let status = unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), &mut result) };

        // Release the returned attributes dictionary (if any).
        if !result.is_null() {
            unsafe { CFRelease(result) };
        }

        if status == errSecItemNotFound {
            return Ok(false);
        }
        if status != 0 {
            return Err(AppError::Keychain(format!(
                "Keychain query failed (OSStatus {})",
                status
            )));
        }
    }
    Ok(true)
}

// Non-macOS stubs
#[cfg(not(target_os = "macos"))]
pub fn store_key(_service: &str, _account: &str, _key: &[u8]) -> Result<(), AppError> {
    Err(AppError::Keychain(
        "Keychain operations are only supported on macOS".to_string(),
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn retrieve_key(_service: &str, _account: &str) -> Result<Vec<u8>, AppError> {
    Err(AppError::Keychain(
        "Keychain operations are only supported on macOS".to_string(),
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn delete_key(_service: &str, _account: &str) -> Result<(), AppError> {
    Err(AppError::Keychain(
        "Keychain operations are only supported on macOS".to_string(),
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn store_metadata(_service: &str, _account: &str, _data: &[u8]) -> Result<(), AppError> {
    Err(AppError::Keychain(
        "Keychain operations are only supported on macOS".to_string(),
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn retrieve_metadata(_service: &str, _account: &str) -> Result<Vec<u8>, AppError> {
    Err(AppError::Keychain(
        "Keychain operations are only supported on macOS".to_string(),
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn delete_metadata(_service: &str, _account: &str) -> Result<(), AppError> {
    Err(AppError::Keychain(
        "Keychain operations are only supported on macOS".to_string(),
    ))
}

#[cfg(not(target_os = "macos"))]
pub fn keys_exist(_service: &str) -> Result<bool, AppError> {
    Err(AppError::Keychain(
        "Keychain operations are only supported on macOS".to_string(),
    ))
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    const TEST_SERVICE: &str = "ch.dokassist.app.test";

    #[test]
    #[ignore = "requires Touch ID hardware"]
    fn test_store_retrieve_delete() {
        let account = "test-key-srd";
        let key = b"test_secret_key_12345678901234567890";

        store_key(TEST_SERVICE, account, key).unwrap();
        let retrieved = retrieve_key(TEST_SERVICE, account).unwrap();
        assert_eq!(key.to_vec(), retrieved);

        delete_key(TEST_SERVICE, account).unwrap();

        let result = retrieve_key(TEST_SERVICE, account);
        assert!(result.is_err());
    }

    #[test]
    #[ignore = "requires Touch ID hardware"]
    fn test_overwrite_key() {
        let account = "test-key-ow";
        let key1 = b"first_key_12345678901234567890123";
        let key2 = b"second_key_0987654321098765432109";

        store_key(TEST_SERVICE, account, key1).unwrap();
        store_key(TEST_SERVICE, account, key2).unwrap();

        let retrieved = retrieve_key(TEST_SERVICE, account).unwrap();
        assert_eq!(key2.to_vec(), retrieved);

        let _ = delete_key(TEST_SERVICE, account);
    }

    /// `keys_exist` for absent items must return `false` without prompting Touch ID.
    #[test]
    fn test_keys_exist_nonexistent() {
        let result = keys_exist("ch.dokassist.app.test.nonexistent");
        assert!(matches!(result, Ok(false)));
    }
}
