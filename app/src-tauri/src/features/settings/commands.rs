use serde::Serialize;
use tauri::{AppHandle, State};

use super::emit_settings;
use crate::core::db::{with_store, Db, StoreProblem};
use crate::core::error::CommandError;

#[tauri::command]
pub fn settings_get(state: State<'_, Db>) -> Result<serde_json::Map<String, serde_json::Value>, CommandError> {
    with_store(&state, |store| store.settings())
}

#[tauri::command]
pub fn settings_set(
    app: AppHandle,
    state: State<'_, Db>,
    key: String,
    value: serde_json::Value,
) -> Result<(), CommandError> {
    with_store(&state, |store| store.set_setting(&key, &value))?;
    emit_settings(&app, &state).map(|_| ())
}

/// Vuelve al valor inicial esas claves; lista vacía = todos los ajustes.
#[tauri::command]
pub fn settings_reset(
    app: AppHandle,
    state: State<'_, Db>,
    keys: Vec<String>,
) -> Result<serde_json::Map<String, serde_json::Value>, CommandError> {
    with_store(&state, |store| store.reset_settings(&keys))?;
    emit_settings(&app, &state)
}

/// Dónde está la base, o por qué se trabaja en memoria (para mostrarlo en Ajustes).
#[derive(Serialize)]
pub struct StoreStatus {
    path: Option<String>,
    problem: Option<StoreProblem>,
}

#[tauri::command]
pub fn store_status(state: State<'_, Db>) -> StoreStatus {
    let guard = state.0.lock().unwrap();
    let store = guard.as_ref();
    StoreStatus {
        path: store.and_then(|s| s.path()).map(|p| p.display().to_string()),
        problem: store.and_then(|s| s.problem()).cloned(),
    }
}
