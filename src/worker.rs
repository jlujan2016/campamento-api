use sqlx::PgPool;
use std::time::Duration;
use crate::telegram::{build_message, TelegramBot};

// Este worker corre en segundo plano cada 30 segundos
// Lee las notificaciones pendientes y las manda a Telegram
pub async fn run_notification_worker(pool: PgPool, bot: TelegramBot) {
    tracing::info!("🤖 Worker de notificaciones iniciado");

    loop {
        // Esperamos 30 segundos entre cada ciclo
        tokio::time::sleep(Duration::from_secs(30)).await;

        match process_pending_notifications(&pool, &bot).await {
            Ok(count) if count > 0 => {
                tracing::info!("📨 {} notificaciones enviadas", count);
            }
            Err(e) => {
                tracing::error!("Error procesando notificaciones: {:?}", e);
            }
            _ => {} // 0 notificaciones, no logueamos nada
        }

        // También verificamos turnos que empiezan en ~1 hora para recordatorios
        if let Err(e) = schedule_shift_reminders(&pool).await {
            tracing::error!("Error programando recordatorios: {:?}", e);
        }
    }
}

async fn process_pending_notifications(
    pool: &PgPool,
    bot: &TelegramBot,
) -> Result<usize, sqlx::Error> {

    // Tomamos hasta 10 notificaciones pendientes a la vez
    let notifications = sqlx::query!(
        r#"
        SELECT id, event_id, user_id, type as notification_type, payload
        FROM notifications
        WHERE status = 'pending'
        ORDER BY created_at ASC
        LIMIT 10
        "#
    )
    .fetch_all(pool)
    .await?;

    let mut sent_count = 0;

    for notif in &notifications {
        // Determinamos a qué chat mandar según el tipo
        let chat_ids = get_target_chat_ids(
            pool,
            notif.event_id,
            notif.user_id,
            &notif.notification_type,
        ).await?;

        if chat_ids.is_empty() {
            // No hay chat vinculado — marcamos como enviado igual para no reintentar
            mark_notification(pool, notif.id, "sent").await?;
            continue;
        }

        let message = build_message(&notif.notification_type, &notif.payload);
        let mut all_sent = true;

        for chat_id in &chat_ids {
            if let Err(e) = bot.send_message(chat_id, &message).await {
                tracing::warn!("No se pudo enviar a {}: {:?}", chat_id, e);
                all_sent = false;
            }
        }

        let new_status = if all_sent { "sent" } else { "failed" };
        mark_notification(pool, notif.id, new_status).await?;

        if all_sent {
            sent_count += 1;
        }
    }

    Ok(sent_count)
}

// Determina qué chat_ids deben recibir esta notificación
async fn get_target_chat_ids(
    pool: &PgPool,
    event_id: Option<uuid::Uuid>,
    user_id: Option<uuid::Uuid>,
    notification_type: &str,
) -> Result<Vec<String>, sqlx::Error> {

    // Notificaciones grupales — van al canal del evento
    let is_group_notification = matches!(
        notification_type,
        "slot_freed" | "gap_unresolved" | "new_schedule_link"
    );

    if is_group_notification {
        if let Some(event_id) = event_id {
            let chats = sqlx::query!(
                r#"
                SELECT telegram_chat_id FROM telegram_links
                WHERE event_id = $1 AND link_type = 'group'
                "#,
                event_id
            )
            .fetch_all(pool)
            .await?;

            return Ok(chats.into_iter().map(|r| r.telegram_chat_id).collect());
        }
    }

    // Notificaciones privadas — van al usuario específico
    if let Some(user_id) = user_id {
        let chats = sqlx::query!(
            r#"
            SELECT telegram_chat_id FROM telegram_links
            WHERE user_id = $1 AND link_type = 'private'
            "#,
            user_id
        )
        .fetch_all(pool)
        .await?;

        return Ok(chats.into_iter().map(|r| r.telegram_chat_id).collect());
    }

    Ok(vec![])
}

async fn mark_notification(
    pool: &PgPool,
    notification_id: uuid::Uuid,
    status: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "UPDATE notifications SET status = $1, sent_at = NOW() WHERE id = $2",
        status,
        notification_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

// Programa recordatorios para turnos que empiezan en ~1 hora
async fn schedule_shift_reminders(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Buscamos turnos aprobados que empiecen entre 55 y 65 minutos
    // (ventana de 10 min para no mandar el mismo recordatorio dos veces)
    let upcoming_shifts = sqlx::query!(
        r#"
        SELECT s.id, s.event_id, s.user_id, s.scheduled_start
        FROM shifts s
        WHERE s.status = 'approved'
        AND s.scheduled_start BETWEEN NOW() + INTERVAL '55 minutes'
                                  AND NOW() + INTERVAL '65 minutes'
        AND NOT EXISTS (
            SELECT 1 FROM notifications n
            WHERE n.user_id = s.user_id
            AND n.type = 'shift_reminder'
            AND (n.payload->>'shift_id')::uuid = s.id
        )
        "#
    )
    .fetch_all(pool)
    .await?;

    for shift in upcoming_shifts {
        sqlx::query!(
            r#"
            INSERT INTO notifications (event_id, user_id, type, payload)
            VALUES ($1, $2, 'shift_reminder', $3)
            "#,
            shift.event_id,
            shift.user_id,
            serde_json::json!({
                "shift_id": shift.id,
                "start": shift.scheduled_start.format("%d/%m %H:%M").to_string()
            })
        )
        .execute(pool)
        .await?;
    }

    Ok(())
}