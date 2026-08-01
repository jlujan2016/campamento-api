use serde::Serialize;
use crate::errors::AppError;

// Cliente para la API de Telegram
#[derive(Clone)]
pub struct TelegramBot {
    token: String,
    client: reqwest::Client,
}

// Payload para sendMessage de la API de Telegram
#[derive(Serialize)]
struct SendMessagePayload {
    chat_id: String,
    text: String,
    parse_mode: String,  // "HTML" o "Markdown"
}

impl TelegramBot {
    pub fn new(token: String) -> Self {
        Self {
            token,
            client: reqwest::Client::new(),
        }
    }

    // Manda un mensaje a cualquier chat_id (grupo o privado)
    pub async fn send_message(&self, chat_id: &str, text: &str) -> Result<(), AppError> {
        let url = format!(
            "https://api.telegram.org/bot{}/sendMessage",
            self.token
        );

        let payload = SendMessagePayload {
            chat_id: chat_id.to_string(),
            text: text.to_string(),
            parse_mode: "HTML".to_string(),
        };

        let response = self.client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| AppError::Validation(format!("Error enviando a Telegram: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            tracing::warn!("Telegram API error: {}", error_text);
            // No propagamos el error — si Telegram falla, el sistema sigue andando
        }

        Ok(())
    }
}

// Genera el texto del mensaje según el tipo de notificación
pub fn build_message(notification_type: &str, payload: &serde_json::Value) -> String {
    match notification_type {
        "slot_freed" => {
            let count = payload["freed_count"].as_u64().unwrap_or(0);
            let msg = payload["message"].as_str().unwrap_or("");
            format!(
                "🔔 <b>Turnos disponibles</b>\n\n\
                Se liberaron <b>{}</b> turno(s).\n{}\n\n\
                ¡Anotate en el cronograma si podés cubrir!",
                count, msg
            )
        },
        "gap_unresolved" => {
            format!(
                "⚠️ <b>Turno sin cubrir</b>\n\n\
                Hay un turno que quedó sin cubrir y necesita atención.\n\
                Por favor coordinen quién puede cubrirlo."
            )
        },
        "new_schedule_link" => {
            let link = payload["link"].as_str().unwrap_or("");
            format!(
                "📅 <b>Nuevo cronograma disponible</b>\n\n\
                Se generó un enlace para anotarse en los turnos:\n\
                {}\n\n\
                El enlace expira en 72 horas.",
                link
            )
        },
        "extra_approved" => {
            let start = payload["start"].as_str().unwrap_or("");
            let end = payload["end"].as_str().unwrap_or("");
            format!(
                "✅ <b>Turno extra aprobado</b>\n\n\
                Tu turno extra del {} al {} fue aprobado.\n\
                Recordá hacer check-in cuando llegues.",
                start, end
            )
        },
        "contribution_approved" => {
            let label = payload["label"].as_str().unwrap_or("aporte");
            let bonus = payload["hour_bonus"].as_f64().unwrap_or(0.0);
            format!(
                "✅ <b>Aporte aprobado</b>\n\n\
                Tu aporte de <b>{}</b> fue aprobado.\n\
                Sumás <b>{:.1} horas</b> a tu puntaje.",
                label, bonus
            )
        },
        "replacement_confirmed" => {
            let original = payload["original_name"].as_str().unwrap_or("");
            let replacement = payload["replacement_name"].as_str().unwrap_or("");
            let start = payload["start"].as_str().unwrap_or("");
            format!(
                "🔄 <b>Reemplazo confirmado</b>\n\n\
                <b>{}</b> cubrirá el turno de <b>{}</b>\n\
                Horario: {}",
                replacement, original, start
            )
        },
        "shift_reminder" => {
            let start = payload["start"].as_str().unwrap_or("");
            format!(
                "⏰ <b>Recordatorio de turno</b>\n\n\
                Tu turno empieza en 1 hora: <b>{}</b>\n\
                No olvides hacer check-in cuando llegues.",
                start
            )
        },
        _ => format!("📢 Notificación del sistema de campamento"),
    }
}

use serde::Deserialize;

// ── Estructuras para leer los mensajes entrantes (getUpdates) ──

#[derive(Debug, Deserialize)]
struct TelegramUpdate {
    update_id: i64,
    message: Option<TelegramMessage>,
}

#[derive(Debug, Deserialize)]
struct TelegramMessage {
    chat: TelegramChat,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TelegramChat {
    id: i64,
    #[serde(rename = "type")]
    chat_type: String,
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetUpdatesResponse {
    result: Vec<TelegramUpdate>,
}

impl TelegramBot {
    // Revisa mensajes nuevos y responde automáticamente al comando /chatid
    // offset: desde qué update_id seguir (evita repetir mensajes ya procesados)
    // Devuelve el próximo offset a usar en la siguiente llamada
    pub async fn check_chatid_commands(&self, offset: i64) -> i64 {
        let url = format!(
            "https://api.telegram.org/bot{}/getUpdates?offset={}&timeout=0",
            self.token, offset
        );

        let resp = match self.client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("Error consultando getUpdates: {}", e);
                return offset;
            }
        };

        let data: GetUpdatesResponse = match resp.json().await {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!("Error parseando getUpdates: {}", e);
                return offset;
            }
        };

        let mut next_offset = offset;

        for update in data.result {
            // El próximo offset siempre es el último update_id visto + 1
            next_offset = update.update_id + 1;

            if let Some(msg) = update.message {
                let is_chatid_command = msg.text
                    .as_deref()
                    .map(|t| t.trim() == "/chatid")
                    .unwrap_or(false);

                if is_chatid_command {
                    let reply = if msg.chat.chat_type == "private" {
                        format!(
                            "🆔 Tu Chat ID privado es:\n\n<code>{}</code>\n\n\
                            Esto es solo informativo — no necesitás pegarlo en ningún lado. \
                            Tu cuenta ya está vinculada automáticamente cuando usaste /start.",
                            msg.chat.id
                        )
                    } else {
                        let group_name = msg.chat.title
                            .clone()
                            .unwrap_or_else(|| "sin nombre".to_string());
                        format!(
                            "🆔 El Chat ID de este grupo (\"{}\") es:\n\n<code>{}</code>\n\n\
                            Copiá ese número y pegalo en la app, en \
                            Configuración del evento → Notificaciones Telegram → Vincular.",
                            group_name, msg.chat.id
                        )
                    };

                    let chat_id_str = msg.chat.id.to_string();
                    if let Err(e) = self.send_message(&chat_id_str, &reply).await {
                        tracing::warn!("Error respondiendo /chatid: {:?}", e);
                    }
                }
            }
        }

        next_offset
    }
}