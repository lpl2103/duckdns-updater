//! # Módulo de Webhooks e Notificações Externas (`core/webhook.rs`)
//!
//! Envia alertas em segundo plano para Discord e Telegram quando o endereço IP público for atualizado
//! ou em caso de falha crítica.

use crate::core::config::AppConfig;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct WebhookEvent {
    pub domains: String,
    pub old_ipv4: Option<String>,
    pub new_ipv4: Option<String>,
    pub old_ipv6: Option<String>,
    pub new_ipv6: Option<String>,
    pub success: bool,
    pub message: String,
    pub timestamp: String,
}

/// Dispara notificações para todos os canais configurados (Discord, Telegram).
pub fn send_notifications(config: &AppConfig, event: &WebhookEvent) {
    if !config.discord_webhook.trim().is_empty() {
        let _ = send_discord(&config.discord_webhook, event);
    }

    if !config.telegram_bot_token.trim().is_empty() && !config.telegram_chat_id.trim().is_empty() {
        let _ = send_telegram(
            &config.telegram_bot_token,
            &config.telegram_chat_id,
            event,
        );
    }
}

/// Envia uma notificação de teste para verificar se o Webhook está operando.
pub fn test_webhook(config: &AppConfig) -> Result<String, String> {
    let mut sent = Vec::new();

    let test_event = WebhookEvent {
        domains: if config.domains_csv().is_empty() {
            "exemplo.duckdns.org".to_string()
        } else {
            config.domains_csv().into_owned()
        },
        old_ipv4: Some("127.0.0.1".to_string()),
        new_ipv4: Some(
            config
                .last_ipv4
                .clone()
                .unwrap_or_else(|| "177.89.105.242".to_string()),
        ),
        old_ipv6: None,
        new_ipv6: config.last_ipv6.clone(),
        success: true,
        message: "Teste de notificação enviado com sucesso pelo DuckDNS Updater!".to_string(),
        timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
    };

    if !config.discord_webhook.trim().is_empty() {
        send_discord(&config.discord_webhook, &test_event)
            .map_err(|e| format!("Falha no Discord: {}", e))?;
        sent.push("Discord");
    }

    if !config.telegram_bot_token.trim().is_empty() && !config.telegram_chat_id.trim().is_empty() {
        send_telegram(
            &config.telegram_bot_token,
            &config.telegram_chat_id,
            &test_event,
        )
        .map_err(|e| format!("Falha no Telegram: {}", e))?;
        sent.push("Telegram");
    }

    if sent.is_empty() {
        Err("Nenhum webhook (Discord ou Telegram) configurado para teste.".to_string())
    } else {
        Ok(format!("Notificação enviada com sucesso para: {}", sent.join(", ")))
    }
}

fn send_discord(webhook_url: &str, event: &WebhookEvent) -> Result<(), String> {
    let color = if event.success { 3066993 } else { 15158332 }; // Verde ou Vermelho
    let title = if event.success {
        "🦆 DuckDNS - IP Público Atualizado"
    } else {
        "⚠ DuckDNS - Falha na Atualização"
    };

    let ipv4_text = event.new_ipv4.as_deref().unwrap_or("N/A");
    let old_ipv4_text = event.old_ipv4.as_deref().unwrap_or("N/A");

    let mut fields = vec![
        serde_json::json!({ "name": "Domínios", "value": format!("`{}`", event.domains), "inline": false }),
        serde_json::json!({ "name": "Novo IPv4", "value": format!("`{}`", ipv4_text), "inline": true }),
        serde_json::json!({ "name": "IPv4 Anterior", "value": format!("`{}`", old_ipv4_text), "inline": true }),
    ];

    if let Some(ref v6) = event.new_ipv6 {
        fields.push(serde_json::json!({ "name": "Novo IPv6", "value": format!("`{}`", v6), "inline": true }));
    }
    if let Some(ref old_v6) = event.old_ipv6 {
        fields.push(serde_json::json!({ "name": "IPv6 Anterior", "value": format!("`{}`", old_v6), "inline": true }));
    }

    fields.push(serde_json::json!({ "name": "Data e Hora", "value": &event.timestamp, "inline": false }));

    let payload = serde_json::json!({
        "username": "DuckDNS Updater",
        "embeds": [{
            "title": title,
            "color": color,
            "fields": fields,
            "footer": { "text": "DuckDNS Updater nativo Windows" }
        }]
    });

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(6))
        .build();

    let resp = agent
        .post(webhook_url)
        .set("Content-Type", "application/json")
        .send_json(payload)
        .map_err(|e| format!("Erro HTTP ao enviar webhook Discord: {}", e))?;

    if resp.status() >= 200 && resp.status() < 300 {
        Ok(())
    } else {
        Err(format!("Discord retornou código de erro HTTP {}", resp.status()))
    }
}

fn send_telegram(bot_token: &str, chat_id: &str, event: &WebhookEvent) -> Result<(), String> {
    let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token.trim());

    let status_icon = if event.success { "✅" } else { "❌" };
    let ipv6_line = match &event.new_ipv6 {
        Some(v6) => format!("\n*Novo IPv6:* `{}`", v6),
        None => String::new(),
    };

    let text = format!(
        "{status_icon} *DuckDNS Updater*\n\n\
        *Domínio(s):* `{}`\n\
        *Novo IPv4:* `{}`\n\
        *IPv4 Anterior:* `{}`{}\n\
        *Status:* {}\n\
        *Data/Hora:* `{}`",
        event.domains,
        event.new_ipv4.as_deref().unwrap_or("N/A"),
        event.old_ipv4.as_deref().unwrap_or("N/A"),
        ipv6_line,
        event.message,
        event.timestamp,
    );

    let payload = serde_json::json!({
        "chat_id": chat_id.trim(),
        "text": text,
        "parse_mode": "Markdown"
    });

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(6))
        .build();

    let resp = agent
        .post(&url)
        .set("Content-Type", "application/json")
        .send_json(payload)
        .map_err(|e| format!("Erro HTTP ao enviar para o Telegram: {}", e))?;

    if resp.status() >= 200 && resp.status() < 300 {
        Ok(())
    } else {
        Err(format!("Telegram retornou código de erro HTTP {}", resp.status()))
    }
}
