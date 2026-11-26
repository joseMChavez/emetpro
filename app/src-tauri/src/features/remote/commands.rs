use std::sync::atomic::Ordering;

use qrcode::render::svg;
use qrcode::QrCode;
use serde::Serialize;
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, State};
use tokio::sync::watch;

use super::board::Op;
use super::chat;
use super::domain::url;
use super::server::{self, Shared};
use super::{save_key, RemoteState, Running};
use crate::core::error::CommandError;

/// Lo que la consola muestra en Ajustes.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteInfo {
    running: bool,
    /// Dirección completa, con la clave (la del QR).
    url: Option<String>,
    /// El QR de `url`, como SVG.
    qr: Option<String>,
    /// false si no se encontró red: el teléfono no podrá llegar.
    on_network: bool,
    clients: usize,
}

fn info(state: &RemoteState) -> RemoteInfo {
    let clients = state.clients.load(Ordering::SeqCst);
    let Some(port) = state.running.lock().unwrap().as_ref().map(|r| r.port) else {
        return RemoteInfo { running: false, url: None, qr: None, on_network: false, clients };
    };
    let ip = server::lan_ip();
    let address = url(&ip.to_string(), port, &state.key.borrow());
    let qr = QrCode::new(address.as_bytes())
        .ok()
        .map(|code| code.render::<svg::Color>().min_dimensions(220, 220).quiet_zone(true).build());
    RemoteInfo { running: true, url: Some(address), qr, on_network: !ip.is_loopback(), clients }
}

#[tauri::command]
pub fn remote_info(state: State<'_, RemoteState>) -> RemoteInfo {
    info(&state)
}

#[tauri::command]
pub async fn remote_start(app: AppHandle, state: State<'_, RemoteState>) -> Result<RemoteInfo, CommandError> {
    if state.running.lock().unwrap().is_some() {
        return Ok(info(&state));
    }
    let (stop, stop_rx) = watch::channel(false);
    let shared = Shared {
        app,
        key: state.key.subscribe(),
        status: state.status.subscribe(),
        stop: stop_rx,
        clients: state.clients.clone(),
        thumbs: state.thumbs.clone(),
        thumbs_added: state.thumbs_added.subscribe(),
        board: state.board.clone(),
        chat: state.chat.clone(),
    };
    let port = server::start(shared).await.map_err(|e| CommandError::RemoteFailed { reason: e.to_string() })?;
    let mut running = state.running.lock().unwrap();
    if running.is_none() {
        *running = Some(Running { port, _stop: stop });
    }
    drop(running);
    Ok(info(&state))
}

#[tauri::command]
pub fn remote_stop(app: AppHandle, state: State<'_, RemoteState>) -> RemoteInfo {
    // Soltar `stop` apaga el servidor y desconecta los teléfonos. La pizarra dura lo que dura el control remoto.
    if state.running.lock().unwrap().take().is_some() {
        state.board.reset(&app);
        state.chat.update(&app, |c| c.reset(), |_| true);
    }
    info(&state)
}

/// Clave nueva: los teléfonos conectados se desconectan y hay que volver a escanear el QR.
#[tauri::command]
pub fn remote_reset_key(app: AppHandle, state: State<'_, RemoteState>) -> RemoteInfo {
    state.key.send_replace(save_key(&app));
    info(&state)
}

/// La consola publica lo que hay en pantalla; se reenvía tal cual a los teléfonos.
#[tauri::command]
pub fn remote_publish(state: State<'_, RemoteState>, status: serde_json::Value) {
    state.status.send_replace(serde_json::json!({ "type": "status", "status": status }).to_string());
}

/// Otro PDF en pantalla, de `total` páginas: se olvidan las miniaturas del anterior.
#[tauri::command]
pub fn remote_thumbs_start(state: State<'_, RemoteState>, doc: String, total: u32) -> bool {
    state.thumbs.lock().unwrap().start(&doc, total)
}

/// Una miniatura dibujada por la consola. Llega como bytes (JPEG) y con el documento y la página en cabeceras,
/// para no pasar las imágenes por JSON. false si se descarta (otro documento, o no es una miniatura).
#[tauri::command]
pub fn remote_thumb(state: State<'_, RemoteState>, request: Request<'_>) -> bool {
    let header = |name: &str| request.headers().get(name).and_then(|v| v.to_str().ok());
    let (Some(doc), Some(page)) = (header("x-doc"), header("x-page").and_then(|p| p.parse::<u32>().ok())) else {
        return false;
    };
    let InvokeBody::Raw(jpeg) = request.body() else { return false };
    let saved = state.thumbs.lock().unwrap().put(doc, page, jpeg.clone());
    if saved {
        state.thumbs_added.send_modify(|n| *n = n.wrapping_add(1));
    }
    saved
}

/// La pizarra entera (board.rs), para la consola y la salida al abrirse.
#[tauri::command]
pub fn remote_board(state: State<'_, RemoteState>) -> serde_json::Value {
    state.board.board()
}

/// Un cambio en la pizarra desde la consola (quitar lo dibujado). Se reparte igual que los del teléfono.
#[tauri::command]
pub fn remote_board_op(app: AppHandle, state: State<'_, RemoteState>, op: serde_json::Value) -> bool {
    match Op::parse(&op.to_string()) {
        Some(op) => {
            state.board.apply(&app, op);
            true
        }
        None => false,
    }
}

/// La ventana de salida cuenta cómo es la pantalla y dónde quedó cada palabra, para dibujar encima desde el teléfono.
#[tauri::command]
pub fn remote_screen(state: State<'_, RemoteState>, screen: Option<serde_json::Value>) {
    state.board.set_screen(screen);
}

/// Los mensajes con los teléfonos y quién está conectado (chat.rs).
#[tauri::command]
pub fn remote_chat(state: State<'_, RemoteState>) -> serde_json::Value {
    state.chat.console_view()
}

/// Un mensaje de la consola para un teléfono (`to`) o para todos. Devuelve su id; None si no vale.
#[tauri::command]
pub fn remote_chat_send(app: AppHandle, state: State<'_, RemoteState>, to: Option<String>, text: String, replies: Vec<String>) -> Option<String> {
    state.chat.update(&app, |c| c.send(to.as_deref(), &text, &replies, chat::now()), Option::is_some)
}

/// Retirar un mensaje de la consola, o dar por leído uno de un teléfono.
#[tauri::command]
pub fn remote_chat_close(app: AppHandle, state: State<'_, RemoteState>, id: String) -> bool {
    state.chat.update(&app, |c| c.close(&id), |closed| *closed)
}

/// Las frases que el teléfono ofrece para mandar a la consola.
#[tauri::command]
pub fn remote_chat_phrases(app: AppHandle, state: State<'_, RemoteState>, phrases: Vec<String>) {
    state.chat.update(&app, |c| c.set_phrases(&phrases), |changed| *changed);
}

/// Aviso del sistema (el de Windows, macOS o Linux) de un mensaje que llegó del remoto. Sin `always`, solo si la
/// consola no está al frente: tapada por la pantalla de salida, minimizada o con otra aplicación delante (con la
/// consola delante ya se ve arriba, en la lista de avisos). Devuelve si se mostró.
#[tauri::command]
pub fn remote_notify(app: AppHandle, title: String, body: String, always: bool) -> bool {
    use tauri::Manager;
    use tauri_plugin_notification::NotificationExt;
    let in_front = app
        .get_webview_window(crate::core::events::MAIN_LABEL)
        .is_some_and(|w| w.is_focused().unwrap_or(false) && !w.is_minimized().unwrap_or(false));
    if in_front && !always {
        return false;
    }
    let (Some(title), Some(body)) = (short(&title, NOTIFY_TITLE), short(&body, NOTIFY_BODY)) else {
        return false;
    };
    app.notification().builder().title(title).body(body).show().is_ok()
}

const NOTIFY_TITLE: usize = chat::MAX_NAME + 40;
const NOTIFY_BODY: usize = chat::MAX_TEXT + chat::MAX_REPLY + 40;

/// Recortado a `max` caracteres (con «…»), sin caracteres de control; None si queda vacío.
fn short(text: &str, max: usize) -> Option<String> {
    let mut chars = text.chars().map(|c| if c.is_control() { ' ' } else { c });
    let mut out: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        out.push('…');
    }
    chat::clean(&out, max + 1)
}
