use std::fmt::Write;

use wasm_bindgen::JsValue;
use web_sys::Storage;

use shared::settings::{SettingKey, SettingValue, SettingsStore};

const STORAGE_KEY_PREFIX: &str = "bacter.";

/// localStorage holds strings, so each value's bytes are stored as hexadecimal.
pub struct LocalStorageSettingsStore;

impl SettingsStore for LocalStorageSettingsStore {
    fn load<K: SettingKey>(&self, key: K) -> Option<SettingValue> {
        let storage: Storage = get_local_storage()?;
        let stored_text: String = storage.get_item(&get_storage_key(key)).ok()??;
        let bytes: Vec<u8> = decode_hexadecimal(&stored_text)?;

        SettingValue::from_bytes(&bytes)
    }

    fn store<K: SettingKey>(&self, key: K, value: SettingValue) {
        let Some(storage) = get_local_storage() else {
            log::warn!("settings not stored: localStorage is unavailable");

            return;
        };

        let stored: Result<(), JsValue> =
            storage.set_item(&get_storage_key(key), &encode_hexadecimal(&value.to_bytes()));

        if let Err(error) = stored {
            log::warn!("storing a setting failed; [key={} error={error:?}]", key.storage_key());
        }
    }
}

fn get_local_storage() -> Option<Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn get_storage_key<K: SettingKey>(key: K) -> String {
    format!("{STORAGE_KEY_PREFIX}{}", key.storage_key())
}

fn encode_hexadecimal(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");

        text
    })
}

/// `None` for an odd length or a character outside hexadecimal.
fn decode_hexadecimal(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }

    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(text.get(index..index + 2)?, 16).ok())
        .collect()
}

#[cfg(all(test, target_arch = "wasm32"))] // reads and writes the browser's localStorage
mod tests {
    use super::*;
    use shared::settings;
    use shared::settings::ClientSettingKind;
    use wasm_bindgen_test::wasm_bindgen_test;

    fn clear_stored_settings() {
        let storage: Storage = get_local_storage().unwrap();

        for key in [ClientSettingKind::NameLabels, ClientSettingKind::Messages] {
            storage.remove_item(&get_storage_key(key)).unwrap();
        }
    }

    #[wasm_bindgen_test]
    fn load_returns_the_stored_value() {
        clear_stored_settings();
        let store: LocalStorageSettingsStore = LocalStorageSettingsStore;

        store.store(ClientSettingKind::NameLabels, SettingValue::Bool(false));

        assert_eq!(
            store.load(ClientSettingKind::NameLabels),
            Some(SettingValue::Bool(false)),
        );
        assert!(!settings::NAME_LABELS_SETTING.read(&store));
    }

    #[wasm_bindgen_test]
    fn load_returns_none_for_a_missing_value() {
        clear_stored_settings();

        assert_eq!(LocalStorageSettingsStore.load(ClientSettingKind::Messages), None);
        assert!(settings::MESSAGES_SETTING.read(&LocalStorageSettingsStore));
    }

    #[wasm_bindgen_test]
    fn load_returns_none_for_a_corrupt_value() {
        clear_stored_settings();
        let storage: Storage = get_local_storage().unwrap();

        storage.set_item(&get_storage_key(ClientSettingKind::Messages), "0g").unwrap();

        assert_eq!(LocalStorageSettingsStore.load(ClientSettingKind::Messages), None);
    }
}
