const BOOL_TAG: u8 = 1;
const NAME_LABELS_STORAGE_KEY: &str = "name_labels";
const MESSAGES_STORAGE_KEY: &str = "messages";

pub const NAME_LABELS_SETTING: Setting<ClientSettingKind, bool> = Setting::new(ClientSettingKind::NameLabels, true);
pub const MESSAGES_SETTING: Setting<ClientSettingKind, bool> = Setting::new(ClientSettingKind::Messages, true);

pub trait SettingKey: Copy {
    /// Stable across releases; stored values are found by it.
    fn storage_key(self) -> &'static str;
}

/// A leading tag byte makes the bytes self-describing, so a corrupt or wrong-typed value fails to decode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingValue {
    Bool(bool),
}

impl SettingValue {
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            SettingValue::Bool(value) => vec![BOOL_TAG, u8::from(*value)],
        }
    }

    /// `None` for empty or truncated bytes or an unknown tag.
    pub fn from_bytes(bytes: &[u8]) -> Option<SettingValue> {
        match bytes {
            [BOOL_TAG, 0] => Some(SettingValue::Bool(false)),
            [BOOL_TAG, 1] => Some(SettingValue::Bool(true)),
            _ => None,
        }
    }
}

impl From<bool> for SettingValue {
    fn from(value: bool) -> SettingValue {
        SettingValue::Bool(value)
    }
}

impl TryFrom<SettingValue> for bool {
    type Error = SettingValue;

    fn try_from(value: SettingValue) -> Result<bool, SettingValue> {
        match value {
            SettingValue::Bool(inner) => Ok(inner),
        }
    }
}

/// A persistent store for user settings, addressed by a typed key and value.
pub trait SettingsStore {
    fn load<K: SettingKey>(&self, key: K) -> Option<SettingValue>;
    fn store<K: SettingKey>(&self, key: K, value: SettingValue);
}

/// A key with its default.
pub struct Setting<K, T> {
    key: K,
    default: T,
}

impl<K, T> Setting<K, T>
where
    K: SettingKey,
    T: Clone + Into<SettingValue> + TryFrom<SettingValue>,
{
    pub const fn new(key: K, default: T) -> Setting<K, T> {
        Setting { key, default }
    }

    /// The default when the store has no value or an undecodable one.
    pub fn read(&self, store: &impl SettingsStore) -> T {
        store
            .load(self.key)
            .and_then(|value| T::try_from(value).ok())
            .unwrap_or_else(|| self.default.clone())
    }

    pub fn write(&self, store: &impl SettingsStore, value: T) {
        store.store(self.key, value.into());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientSettingKind {
    NameLabels,
    Messages,
}

impl SettingKey for ClientSettingKind {
    fn storage_key(self) -> &'static str {
        match self {
            ClientSettingKind::NameLabels => NAME_LABELS_STORAGE_KEY,
            ClientSettingKind::Messages => MESSAGES_STORAGE_KEY,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    #[derive(Default)]
    struct InMemorySettingsStore {
        entries: RefCell<BTreeMap<&'static str, Vec<u8>>>,
    }

    impl SettingsStore for InMemorySettingsStore {
        fn load<K: SettingKey>(&self, key: K) -> Option<SettingValue> {
            let bytes: Vec<u8> = self.entries.borrow().get(key.storage_key()).cloned()?;

            SettingValue::from_bytes(&bytes)
        }

        fn store<K: SettingKey>(&self, key: K, value: SettingValue) {
            self.entries.borrow_mut().insert(key.storage_key(), value.to_bytes());
        }
    }

    #[test]
    fn read_returns_the_default_when_absent() {
        let store: InMemorySettingsStore = InMemorySettingsStore::default();

        assert!(NAME_LABELS_SETTING.read(&store));
        assert!(MESSAGES_SETTING.read(&store));
    }

    #[test]
    fn write_then_read_round_trips_through_the_store() {
        let store: InMemorySettingsStore = InMemorySettingsStore::default();

        MESSAGES_SETTING.write(&store, false);
        assert!(!MESSAGES_SETTING.read(&store));
        assert!(NAME_LABELS_SETTING.read(&store));

        MESSAGES_SETTING.write(&store, true);
        assert!(MESSAGES_SETTING.read(&store));
    }

    #[test]
    fn storage_key_differs_per_setting() {
        assert_eq!(ClientSettingKind::NameLabels.storage_key(), "name_labels");
        assert_eq!(ClientSettingKind::Messages.storage_key(), "messages");
    }

    #[test]
    fn from_bytes_restores_every_value() {
        for value in [SettingValue::Bool(true), SettingValue::Bool(false)] {
            assert_eq!(SettingValue::from_bytes(&value.to_bytes()), Some(value));
        }
    }

    #[test]
    fn from_bytes_rejects_empty_truncated_and_unknown_tag_bytes() {
        assert_eq!(SettingValue::from_bytes(&[]), None);
        assert_eq!(SettingValue::from_bytes(&[BOOL_TAG]), None);
        assert_eq!(SettingValue::from_bytes(&[9, 9]), None);
    }

    #[test]
    fn read_falls_back_to_the_default_on_undecodable_bytes() {
        let store: InMemorySettingsStore = InMemorySettingsStore::default();
        store.entries.borrow_mut().insert("name_labels", vec![0xff, 0xff]);

        assert!(NAME_LABELS_SETTING.read(&store));
    }
}
