use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

use crate::paths;

pub fn read_auth(auth_file: &Path) -> BTreeMap<String, Value> {
    match paths::read_json(auth_file) {
        Some(Value::Object(map)) => map.into_iter().collect(),
        _ => BTreeMap::new(),
    }
}

pub fn provider_ids_from_auth(auth_file: &Path) -> Vec<String> {
    let mut ids: Vec<String> = read_auth(auth_file).keys().cloned().collect();
    ids.sort();
    ids
}

pub fn write_auth_patch(auth_file: &Path, provider_id: &str, cred: &Value) {
    let mut current = read_auth(auth_file);
    current.insert(provider_id.to_string(), cred.clone());
    let value = Value::Object(current.into_iter().collect());
    paths::atomic_write_json(auth_file, &value);
}
