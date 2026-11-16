// Videos por HTTP local, no por el protocolo asset.
// En macOS, el protocolo asset pasa por el manejador de esquemas de WKWebView: un video grande (los de iPhone
// pesan GB) hace muchas peticiones por rangos que WebKit cancela al buscar, pausar o silenciar, y responder a
// una ya cancelada puede cerrar la app. Además, una petición sin rango lee el archivo entero a memoria.
// Aquí WebKit (y WebView2, WebKitGTK) usan su red normal: rangos, cancelaciones y archivos de cualquier tamaño.
// Solo escucha en 127.0.0.1, exige una clave aleatoria de esta sesión y solo sirve archivos ya permitidos
// en el protocolo asset (los que el usuario eligió).
use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::OnceLock;

use axum::extract::{Query, Request, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use tauri::{AppHandle, Manager};
use tokio::net::TcpListener;
use tower::ServiceExt;
use tower_http::services::ServeFile;

/// «http://127.0.0.1:<puerto>/media?k=<clave>&p=», listo para añadir la ruta codificada. None si no arrancó.
static BASE: OnceLock<String> = OnceLock::new();

#[derive(Clone)]
struct Server {
    app: AppHandle,
    key: String,
}

#[derive(Deserialize)]
struct MediaQuery {
    k: String,
    p: String,
}

/// Arranca el servidor en un puerto libre. Si falla, los videos siguen por el protocolo asset.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let listener = match TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await {
            Ok(listener) => listener,
            Err(e) => return eprintln!("[medios] no se pudo abrir el servidor local: {e}"),
        };
        let Ok(addr) = listener.local_addr() else { return };
        let key = uuid::Uuid::new_v4().simple().to_string();
        let _ = BASE.set(format!("http://127.0.0.1:{}/media?k={key}&p=", addr.port()));
        let router = Router::new().route("/media", get(serve).head(serve)).with_state(Server { app, key });
        if let Err(e) = axum::serve(listener, router).await {
            eprintln!("[medios] el servidor local se detuvo: {e}");
        }
    });
}

/// URL local para un archivo ya permitido; None si el servidor no está.
pub fn url(path: &str) -> Option<String> {
    BASE.get().map(|base| format!("{base}{}", encode(path)))
}

async fn serve(State(server): State<Server>, Query(query): Query<MediaQuery>, request: Request) -> Response {
    if query.k != server.key {
        return StatusCode::FORBIDDEN.into_response();
    }
    let path = PathBuf::from(&query.p);
    if !path.is_file() {
        return StatusCode::NOT_FOUND.into_response();
    }
    if !server.app.asset_protocol_scope().is_allowed(&path) {
        return StatusCode::FORBIDDEN.into_response();
    }
    match ServeFile::new(path).oneshot(request).await {
        Ok(response) => response.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Codifica la ruta para la URL (todo menos letras, números y -_.~).
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 3);
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(byte as char),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codifica_rutas() {
        assert_eq!(encode("/Users/ana/IMG_0001.MOV"), "%2FUsers%2Fana%2FIMG_0001.MOV");
        assert_eq!(encode(r"C:\Vídeos\a b&c.mp4"), "C%3A%5CV%C3%ADdeos%5Ca%20b%26c.mp4");
    }
}
