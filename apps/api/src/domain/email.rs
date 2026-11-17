//! Onboarding email delivery (PLAN.md §9.4, superseding the "no SMTP" decision).
//!
//! Delivery is best-effort: if SMTP is not configured or the server refuses the message, the
//! caller still returns the one-time code to the admin so onboarding can never be lost.

use chrono::{DateTime, Utc};
use lettre::message::header::ContentType;
use lettre::message::Mailbox;
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use sqlx::PgPool;

use crate::domain::settings;

/// Masked back to the portal: the real password never leaves the server.
pub const PASSWORD_MASK: &str = "********";

#[derive(Debug, Clone, Default)]
pub struct SmtpConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub from_email: String,
    pub from_name: String,
    pub starttls: bool,
}

impl SmtpConfig {
    /// Safe to attempt a send: enabled, has a host, and has a From address. Credentials are
    /// optional so local capture servers can be used for testing.
    pub fn configured(&self) -> bool {
        self.enabled && !self.host.trim().is_empty() && !self.from_email.trim().is_empty()
    }
}

pub async fn load(pool: &PgPool) -> SmtpConfig {
    let port = settings::get::<i64>(pool, settings::SMTP_PORT)
        .await
        .ok()
        .flatten()
        .unwrap_or(587);
    let enabled = settings::get::<bool>(pool, settings::SMTP_ENABLED)
        .await
        .ok()
        .flatten()
        .unwrap_or(false);
    let starttls = settings::get::<bool>(pool, settings::SMTP_STARTTLS)
        .await
        .ok()
        .flatten()
        .unwrap_or(true);

    SmtpConfig {
        enabled,
        host: text(pool, settings::SMTP_HOST).await,
        port: port.clamp(1, 65535) as u16,
        username: text(pool, settings::SMTP_USERNAME).await,
        password: text(pool, settings::SMTP_PASSWORD).await,
        from_email: text(pool, settings::SMTP_FROM_EMAIL).await,
        from_name: text(pool, settings::SMTP_FROM_NAME).await,
        starttls,
    }
}

async fn text(pool: &PgPool, key: &str) -> String {
    settings::get::<String>(pool, key)
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
}

#[derive(Debug)]
pub enum Delivery {
    Sent,
    Skipped,
    Failed(String),
}

impl Delivery {
    pub fn emailed(&self) -> bool {
        matches!(self, Delivery::Sent)
    }
}

fn mailbox(name: &str, address: &str) -> Result<Mailbox, String> {
    let name = name.trim();
    let address = address.trim();
    let value = if name.is_empty() {
        address.to_string()
    } else {
        format!("{name} <{address}>")
    };
    value.parse::<Mailbox>().map_err(|error| error.to_string())
}

async fn deliver(
    config: &SmtpConfig,
    to_name: &str,
    to_email: &str,
    subject: &str,
    body: String,
) -> Result<(), String> {
    let message = Message::builder()
        .from(mailbox(&config.from_name, &config.from_email)?)
        .to(mailbox(to_name, to_email)?)
        .subject(subject)
        .header(ContentType::TEXT_PLAIN)
        .body(body)
        .map_err(|error| error.to_string())?;

    if config.port == 465 {
        let transport = AsyncSmtpTransport::<Tokio1Executor>::relay(&config.host)
            .map_err(|error| error.to_string())?
            .credentials(Credentials::new(
                config.username.clone(),
                config.password.clone(),
            ))
            .build();
        transport
            .send(message)
            .await
            .map_err(|error| error.to_string())?;
    } else {
        let tls = if config.starttls {
            Tls::Required(
                TlsParameters::new_rustls(config.host.clone())
                    .map_err(|error| error.to_string())?,
            )
        } else {
            Tls::None
        };
        let transport = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host)
            .port(config.port)
            .tls(tls)
            .credentials(Credentials::new(
                config.username.clone(),
                config.password.clone(),
            ))
            .build();
        transport
            .send(message)
            .await
            .map_err(|error| error.to_string())?;
    }

    Ok(())
}

/// Sends a one-time onboarding code. Never fails the caller: a bad relay is reported via
/// `Delivery::Failed` so it can be shown in the portal while the code is still handed over.
pub async fn onboarding(
    pool: &PgPool,
    to_name: &str,
    to_email: &str,
    code: &str,
    expires_at: DateTime<Utc>,
) -> Delivery {
    let config = load(pool).await;
    if !config.configured() {
        return Delivery::Skipped;
    }

    let expires = expires_at.format("%d %b %Y, %H:%M UTC").to_string();
    let body = format!(
        "Hi {to_name},\n\n\
         Your App Directory account has been created. To set your password, open \
         \"App Directory\" on your computer, choose \"Forgot your password?\", then enter \
         this code:\n\n\
             {code}\n\n\
         Your email address is {to_email}. The code expires on {expires} and can be used once.\n\n\
         If you did not expect this message, contact IT.\n"
    );

    match deliver(
        &config,
        to_name,
        to_email,
        "Your App Directory access code",
        body,
    )
    .await
    {
        Ok(()) => Delivery::Sent,
        Err(error) => Delivery::Failed(error),
    }
}

/// Used by the "Send test email" button in Settings.
pub async fn test(pool: &PgPool, to_email: &str) -> Result<(), String> {
    let config = load(pool).await;
    if !config.configured() {
        return Err("SMTP is not enabled, or the host/From address is missing.".to_string());
    }

    deliver(
        &config,
        "App Directory admin",
        to_email,
        "App Directory test email",
        "If you can read this, the App Directory server can send email.\n".to_string(),
    )
    .await
}
