// Mensajes entre la consola y los teléfonos del control remoto: el de la consola avisa al predicador sin
// levantarse («Te quedan 5 minutos», «¿Detengo la transmisión?») y el teléfono responde con un toque; el
// teléfono también puede mandar una frase corta a la consola («Sube el volumen»).
//
// Cada teléfono se presenta con un nombre y un identificador propio que guarda él mismo: así, si se bloquea
// y se reconecta, sigue siendo la misma persona y le vuelve a salir lo que tenía pendiente.
// Todo vive en memoria mientras el control remoto está encendido; al apagarlo se olvida.
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::json;

pub const MAX_NAME: usize = 40;
pub const MAX_TEXT: usize = 300;
pub const MAX_REPLIES: usize = 4;
pub const MAX_REPLY: usize = 40;
const MAX_PHRASES: usize = 16;
const MAX_DEVICE: usize = 64;
/// Lo que se recuerda: al pasar de aquí se olvidan los mensajes cerrados más antiguos.
const MAX_NOTES: usize = 100;
/// Los mensajes propios que ve el teléfono, para saber si la consola los vio.
const MINE_SHOWN: usize = 3;

/// Alguien que se conectó desde un teléfono.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub device: String,
    pub name: String,
    /// Conexiones abiertas (el mismo teléfono puede tener dos pestañas).
    pub online: u32,
    /// Cuándo se conectó o desconectó por última vez (ms desde 1970).
    pub since: u64,
}

/// Un mensaje. De la consola (`from` vacío) a un teléfono o a todos (`to` vacío); o de un teléfono a la consola.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Note {
    pub id: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub text: String,
    /// Las respuestas que se ofrecen; vacío = solo un aviso («Entendido»).
    pub replies: Vec<String>,
    pub at: u64,
    /// Los teléfonos que ya lo vieron en pantalla.
    pub seen: Vec<String>,
    pub answer: Option<Answer>,
    /// Respondido, retirado por la consola o (si es de un teléfono) leído por la consola.
    pub closed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Answer {
    pub by: String,
    /// La respuesta elegida; None = «Entendido» de un aviso.
    pub reply: Option<usize>,
    pub at: u64,
}

/// Lo que dice el teléfono sobre los mensajes. No coincide con ninguna orden (domain::Command) ni con la pizarra.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PhoneMessage {
    /// Al conectarse (y al cambiar de nombre).
    Hello { device: String, name: String },
    Seen { id: String },
    Reply { id: String, reply: Option<usize> },
    /// Una frase para la consola.
    Say { text: String },
}

impl PhoneMessage {
    pub fn parse(text: &str) -> Option<PhoneMessage> {
        let message: PhoneMessage = serde_json::from_str(text).ok()?;
        let valid = match &message {
            PhoneMessage::Hello { device, name } => is_device(device) && clean(name, MAX_NAME).is_some(),
            PhoneMessage::Seen { id } | PhoneMessage::Reply { id, .. } => !id.is_empty() && id.len() <= MAX_DEVICE,
            PhoneMessage::Say { text } => clean(text, MAX_TEXT).is_some(),
        };
        valid.then_some(message)
    }
}

/// El identificador lo inventa el teléfono: letras, números y guiones.
fn is_device(device: &str) -> bool {
    (8..=MAX_DEVICE).contains(&device.len()) && device.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Sin espacios de más ni caracteres de control, y no vacío ni más largo que `max` caracteres.
pub fn clean(text: &str, max: usize) -> Option<String> {
    let text: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    (!text.is_empty() && text.chars().count() <= max).then_some(text)
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

#[derive(Debug, Default)]
pub struct Chat {
    people: Vec<Person>,
    notes: Vec<Note>,
    /// Las frases que el teléfono ofrece para mandar a la consola (las elige la consola, en su idioma).
    phrases: Vec<String>,
    next: u64,
}

impl Chat {
    /// Se conectó (o cambió de nombre) el teléfono `device`. `previous`: con qué identificador se había
    /// presentado esta misma conexión, si ya lo había hecho.
    pub fn hello(&mut self, previous: Option<&str>, device: &str, name: &str, at: u64) -> bool {
        let Some(name) = clean(name, MAX_NAME) else { return false };
        if !is_device(device) {
            return false;
        }
        if let Some(other) = previous.filter(|p| *p != device) {
            self.leave(other, at);
        }
        let again = previous == Some(device);
        match self.people.iter_mut().find(|p| p.device == device) {
            Some(person) => {
                person.name = name;
                if !again {
                    person.online += 1;
                    person.since = at;
                }
            }
            None => self.people.push(Person { device: device.to_string(), name, online: 1, since: at }),
        }
        true
    }

    /// Se cerró una conexión del teléfono `device`.
    pub fn leave(&mut self, device: &str, at: u64) {
        if let Some(person) = self.people.iter_mut().find(|p| p.device == device) {
            person.online = person.online.saturating_sub(1);
            if person.online == 0 {
                person.since = at;
            }
        }
    }

    fn id(&mut self) -> String {
        self.next += 1;
        format!("m{}", self.next)
    }

    /// Un mensaje de la consola. `to`: el teléfono, o None para todos. None si el mensaje no vale.
    pub fn send(&mut self, to: Option<&str>, text: &str, replies: &[String], at: u64) -> Option<String> {
        let text = clean(text, MAX_TEXT)?;
        if replies.len() > MAX_REPLIES || to.is_some_and(|d| !self.people.iter().any(|p| p.device == d)) {
            return None;
        }
        let replies = replies.iter().map(|r| clean(r, MAX_REPLY)).collect::<Option<Vec<_>>>()?;
        let id = self.id();
        self.push(Note { id: id.clone(), from: None, to: to.map(str::to_string), text, replies, at, seen: vec![], answer: None, closed: false });
        Some(id)
    }

    /// Una frase del teléfono `device` para la consola.
    pub fn say(&mut self, device: &str, text: &str, at: u64) -> bool {
        let Some(text) = clean(text, MAX_TEXT) else { return false };
        if !self.people.iter().any(|p| p.device == device) {
            return false;
        }
        let id = self.id();
        self.push(Note { id, from: Some(device.to_string()), to: None, text, replies: vec![], at, seen: vec![], answer: None, closed: false });
        true
    }

    fn push(&mut self, note: Note) {
        self.notes.push(note);
        while self.notes.len() > MAX_NOTES {
            match self.notes.iter().position(|n| n.closed) {
                Some(i) => self.notes.remove(i),
                None => self.notes.remove(0),
            };
        }
    }

    fn for_phone(note: &Note, device: &str) -> bool {
        note.from.is_none() && !note.closed && note.to.as_deref().is_none_or(|to| to == device)
    }

    pub fn seen(&mut self, device: &str, id: &str) -> bool {
        match self.notes.iter_mut().find(|n| n.id == id && Self::for_phone(n, device)) {
            Some(note) if !note.seen.iter().any(|d| d == device) => {
                note.seen.push(device.to_string());
                true
            }
            _ => false,
        }
    }

    /// La primera respuesta vale: el mensaje se cierra y desaparece de los demás teléfonos.
    pub fn reply(&mut self, device: &str, id: &str, reply: Option<usize>, at: u64) -> bool {
        let Some(note) = self.notes.iter_mut().find(|n| n.id == id && Self::for_phone(n, device)) else { return false };
        let valid = match reply {
            Some(i) => i < note.replies.len(),
            None => note.replies.is_empty(),
        };
        if !valid {
            return false;
        }
        if !note.seen.iter().any(|d| d == device) {
            note.seen.push(device.to_string());
        }
        note.answer = Some(Answer { by: device.to_string(), reply, at });
        note.closed = true;
        true
    }

    /// La consola retira un mensaje suyo, o marca como leído uno de un teléfono.
    pub fn close(&mut self, id: &str) -> bool {
        match self.notes.iter_mut().find(|n| n.id == id && !n.closed) {
            Some(note) => {
                note.closed = true;
                true
            }
            None => false,
        }
    }

    pub fn set_phrases(&mut self, phrases: &[String]) -> bool {
        let phrases: Vec<String> = phrases.iter().filter_map(|p| clean(p, MAX_TEXT)).take(MAX_PHRASES).collect();
        if phrases == self.phrases {
            return false;
        }
        self.phrases = phrases;
        true
    }

    /// Al apagar el control remoto. Las frases son de la consola: se quedan.
    pub fn reset(&mut self) {
        self.people.clear();
        self.notes.clear();
    }

    /// Todo, para la consola.
    pub fn console_view(&self) -> serde_json::Value {
        json!({ "people": self.people, "notes": self.notes })
    }

    /// Lo que ve el teléfono `device`: su nombre, lo que tiene pendiente de responder y sus últimas frases.
    pub fn phone_view(&self, device: &str) -> String {
        let name = self.people.iter().find(|p| p.device == device).map(|p| p.name.as_str());
        let pending: Vec<_> = self
            .notes
            .iter()
            .filter(|n| Self::for_phone(n, device))
            .map(|n| json!({ "id": n.id, "text": n.text, "replies": n.replies, "at": n.at, "toAll": n.to.is_none() }))
            .collect();
        let mine: Vec<_> = self
            .notes
            .iter()
            .filter(|n| n.from.as_deref() == Some(device))
            .rev()
            .take(MINE_SHOWN)
            .map(|n| json!({ "id": n.id, "text": n.text, "at": n.at, "read": n.closed }))
            .collect();
        json!({ "type": "chat", "chat": { "name": name, "pending": pending, "mine": mine, "phrases": self.phrases } }).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LUIS: &str = "luis-1234";
    const ANA: &str = "ana-56789";

    fn chat() -> Chat {
        let mut chat = Chat::default();
        assert!(chat.hello(None, LUIS, "Pastor Luis", 1));
        assert!(chat.hello(None, ANA, "  Ana\n ", 1));
        chat
    }

    fn pending(chat: &Chat, device: &str) -> usize {
        let view: serde_json::Value = serde_json::from_str(&chat.phone_view(device)).unwrap();
        view["chat"]["pending"].as_array().unwrap().len()
    }

    #[test]
    fn lee_lo_que_dice_el_telefono() {
        assert_eq!(
            PhoneMessage::parse(r#"{"type":"hello","device":"abcd-1234","name":"Luis"}"#),
            Some(PhoneMessage::Hello { device: "abcd-1234".into(), name: "Luis".into() })
        );
        assert_eq!(PhoneMessage::parse(r#"{"type":"reply","id":"m1","reply":1}"#), Some(PhoneMessage::Reply { id: "m1".into(), reply: Some(1) }));
        assert_eq!(PhoneMessage::parse(r#"{"type":"reply","id":"m1","reply":null}"#), Some(PhoneMessage::Reply { id: "m1".into(), reply: None }));
        assert_eq!(PhoneMessage::parse(r#"{"type":"say","text":"Sube el volumen"}"#), Some(PhoneMessage::Say { text: "Sube el volumen".into() }));
        assert_eq!(PhoneMessage::parse(r#"{"type":"hello","device":"../../x","name":"Luis"}"#), None);
        assert_eq!(PhoneMessage::parse(r#"{"type":"hello","device":"abcd-1234","name":"   "}"#), None);
        assert_eq!(PhoneMessage::parse(&format!(r#"{{"type":"say","text":"{}"}}"#, "x".repeat(400))), None);
        // Las órdenes y la pizarra no se confunden con mensajes.
        assert_eq!(PhoneMessage::parse(r#"{"type":"next"}"#), None);
        assert_eq!(PhoneMessage::parse(r#"{"type":"wipe","layer":"x"}"#), None);
    }

    #[test]
    fn nombres_y_conexiones() {
        let mut chat = chat();
        assert_eq!(chat.people[1].name, "Ana");
        // Reconecta el mismo teléfono con otro nombre: sigue siendo la misma persona.
        assert!(chat.hello(None, LUIS, "Luis", 5));
        assert_eq!(chat.people.len(), 2);
        assert_eq!((chat.people[0].name.as_str(), chat.people[0].online), ("Luis", 2));
        // Cambiar de nombre en la misma conexión no cuenta otra conexión.
        assert!(chat.hello(Some(LUIS), LUIS, "Pastor Luis", 6));
        assert_eq!(chat.people[0].online, 2);
        chat.leave(LUIS, 7);
        chat.leave(LUIS, 8);
        assert_eq!((chat.people[0].online, chat.people[0].since), (0, 8));
        assert!(!chat.hello(None, "x", "Luis", 9));
    }

    #[test]
    fn a_uno_o_a_todos() {
        let mut chat = chat();
        let to_luis = chat.send(Some(LUIS), "Te quedan 5 minutos", &[], 10).unwrap();
        let to_all = chat.send(None, "¿Detengo la transmisión?", &["Sí".into(), "No".into()], 11).unwrap();
        assert_eq!((pending(&chat, LUIS), pending(&chat, ANA)), (2, 1));
        // A alguien que no se ha conectado nunca, no.
        assert_eq!(chat.send(Some("nadie-123"), "Hola", &[], 12), None);
        assert_eq!(chat.send(None, "  ", &[], 12), None);
        assert_eq!(chat.send(None, "Hola", &["a".into(), "b".into(), "c".into(), "d".into(), "e".into()], 12), None);

        // Ana no puede responder lo que era para Luis.
        assert!(!chat.reply(ANA, &to_luis, None, 13));
        // Un sí/no no se responde con «Entendido», ni con una opción que no existe.
        assert!(!chat.reply(ANA, &to_all, None, 13));
        assert!(!chat.reply(ANA, &to_all, Some(2), 13));
        // La primera respuesta vale y se cierra para todos.
        assert!(chat.reply(ANA, &to_all, Some(1), 14));
        assert!(!chat.reply(LUIS, &to_all, Some(0), 15));
        assert_eq!((pending(&chat, LUIS), pending(&chat, ANA)), (1, 0));
        let note = chat.notes.iter().find(|n| n.id == to_all).unwrap();
        assert_eq!(note.answer, Some(Answer { by: ANA.into(), reply: Some(1), at: 14 }));

        assert!(chat.seen(LUIS, &to_luis));
        assert!(!chat.seen(LUIS, &to_luis));
        assert!(chat.reply(LUIS, &to_luis, None, 16));
        assert_eq!(pending(&chat, LUIS), 0);
    }

    #[test]
    fn pendiente_al_reconectar() {
        let mut chat = chat();
        chat.leave(LUIS, 2);
        let id = chat.send(Some(LUIS), "Te quedan 5 minutos", &[], 3).unwrap();
        assert!(chat.hello(None, LUIS, "Pastor Luis", 4));
        assert_eq!(pending(&chat, LUIS), 1);
        // La consola lo retira: deja de salir.
        assert!(chat.close(&id));
        assert!(!chat.close(&id));
        assert_eq!(pending(&chat, LUIS), 0);
    }

    #[test]
    fn frases_para_la_consola() {
        let mut chat = chat();
        assert!(chat.say(LUIS, "Sube el volumen", 5));
        assert!(!chat.say("nadie-123", "Hola", 5));
        let id = chat.notes[0].id.clone();
        let view = |chat: &Chat| -> serde_json::Value { serde_json::from_str(&chat.phone_view(LUIS)).unwrap() };
        assert_eq!(view(&chat)["chat"]["mine"][0]["read"], false);
        // Una frase de un teléfono no le sale a nadie como pendiente.
        assert_eq!(pending(&chat, ANA), 0);
        assert!(chat.close(&id));
        assert_eq!(view(&chat)["chat"]["mine"][0]["read"], true);
        assert!(chat.set_phrases(&["Sube el volumen".into(), " ".into()]));
        assert!(!chat.set_phrases(&["Sube el volumen".into()]));
        assert_eq!(view(&chat)["chat"]["phrases"], json!(["Sube el volumen"]));
    }

    #[test]
    fn no_crece_sin_fin() {
        let mut chat = chat();
        let first = chat.send(None, "primero", &[], 1).unwrap();
        for i in 0..MAX_NOTES + 10 {
            let id = chat.send(None, &format!("n{i}"), &[], 2).unwrap();
            chat.close(&id);
        }
        assert_eq!(chat.notes.len(), MAX_NOTES);
        // Los cerrados se olvidan antes que uno que sigue esperando respuesta.
        assert!(chat.notes.iter().any(|n| n.id == first));
    }

    #[test]
    fn limpia_el_texto() {
        assert_eq!(clean(" Te quedan\n\t5  minutos ", MAX_TEXT).as_deref(), Some("Te quedan 5 minutos"));
        assert_eq!(clean("\u{7}", MAX_TEXT), None);
        assert_eq!(clean(&"ñ".repeat(MAX_NAME), MAX_NAME).map(|s| s.chars().count()), Some(MAX_NAME));
    }
}
