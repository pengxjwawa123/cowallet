use aws_sdk_sesv2::{
    types::{Body, Content, Destination, EmailContent, Message},
    Client as SesClient,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct EmailService {
    inner: Arc<EmailServiceInner>,
}

struct EmailServiceInner {
    client: SesClient,
    from_address: String,
}

impl EmailService {
    pub async fn from_env() -> Option<Self> {
        let from_address = std::env::var("SES_FROM_ADDRESS").ok()?;

        let config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let client = SesClient::new(&config);

        tracing::info!("AWS SES email service initialized (from: {})", from_address);
        Some(Self {
            inner: Arc::new(EmailServiceInner {
                client,
                from_address,
            }),
        })
    }

    pub async fn send_otp(&self, to_email: &str, otp: &str) -> Result<(), String> {
        let html_body = format!(
            r#"<!DOCTYPE html>
<html>
<body style="font-family: -apple-system, sans-serif; padding: 20px;">
  <h2>Wallet Recovery</h2>
  <p>Your verification code is:</p>
  <div style="font-size: 32px; font-weight: bold; letter-spacing: 8px; padding: 16px; background: #f5f5f5; border-radius: 8px; text-align: center;">{otp}</div>
  <p style="color: #666; margin-top: 16px;">This code expires in 10 minutes. Do not share it with anyone.</p>
  <p style="color: #999; font-size: 12px;">If you did not request this, please ignore this email.</p>
</body>
</html>"#
        );

        let text_body = format!(
            "CoWallet Recovery\n\nYour verification code is: {}\n\nThis code expires in 10 minutes.",
            otp
        );

        self.inner
            .client
            .send_email()
            .from_email_address(&self.inner.from_address)
            .destination(
                Destination::builder()
                    .to_addresses(to_email)
                    .build(),
            )
            .content(
                EmailContent::builder()
                    .simple(
                        Message::builder()
                            .subject(
                                Content::builder()
                                    .data("CoWallet - Your Recovery Code")
                                    .charset("UTF-8")
                                    .build()
                                    .unwrap(),
                            )
                            .body(
                                Body::builder()
                                    .html(
                                        Content::builder()
                                            .data(html_body)
                                            .charset("UTF-8")
                                            .build()
                                            .unwrap(),
                                    )
                                    .text(
                                        Content::builder()
                                            .data(text_body)
                                            .charset("UTF-8")
                                            .build()
                                            .unwrap(),
                                    )
                                    .build(),
                            )
                            .build(),
                    )
                    .build(),
            )
            .send()
            .await
            .map_err(|e| {
                tracing::error!("SES raw error: {:?}", e);
                format!("SES send failed: {}", e)
            })?;

        tracing::info!("Recovery OTP email sent to {}", to_email);
        Ok(())
    }
}
