use crate::config::Config;
use crate::delivery_result::DeliveryResult;
use crate::prepared_email::PreparedEmail;
use lettre::smtp::authentication::Credentials;
use lettre::smtp::client::net::ClientTlsParameters;
use lettre::smtp::error::Error as LettreSmtpError;
use lettre::smtp::extension::ClientId;
use lettre::smtp::response::Severity;
use lettre::smtp::{ClientSecurity, SmtpClient};
use lettre::Transport;
use native_tls::{TlsConnector, Protocol};
use std::net::ToSocketAddrs;
use std::time::Duration;

// Deliver an email to an SMTP server
pub fn smtp_delivery(
    prepared_email: &PreparedEmail,
    smtp_server_domain: &str,
    config: &Config
) -> DeliveryResult {
    trace!(
        "SMTP delivery to [{}] at {}",
        prepared_email.to.join(", "),
        smtp_server_domain
    );

    // lettre::EmailAddress checks validity.  But we checked that when we created
    // PreparedEmail so this conversion should always pass.
    let sendable_email = match prepared_email.as_sendable_email() {
        Ok(se) => se,
        Err(e) => {
            warn!("Invalid email address error: {:?}", e);
            return DeliveryResult::Failed(format!("Invalid email address error: {:?}", e));
        }
    };

    let tls_builder = match TlsConnector::builder()
        .min_protocol_version(Some(Protocol::Tlsv12))
        .build()
    {
        Ok(connector) => connector,
        Err(e) => {
            info!("(worker) failed to create TLS Connector: {:?}", e);
            return DeliveryResult::Failed(format!("Failed to create TLS connector: {:?}", e));
        }
    };

    let tls_parameters =
        ClientTlsParameters::new(smtp_server_domain.to_owned(), tls_builder);

    let client_security = if config.require_tls {
        ClientSecurity::Required(tls_parameters)
    } else {
        ClientSecurity::Opportunistic(tls_parameters)
    };

    // Build sockaddr
    let sockaddr = match (smtp_server_domain, 25_u16).to_socket_addrs() {
        Err(e) => {
            warn!(
                "ToSocketAddr failed for ({}, 25): {:?}",
                smtp_server_domain, e
            );
            return DeliveryResult::Failed(format!(
                "ToSockaddr failed for ({}, 25): {:?}",
                smtp_server_domain, e
            ));
        }
        Ok(mut iter) => match iter.next() {
            Some(sa) => sa,
            None => {
                warn!("No SockAddrs for ({}, 25)", smtp_server_domain);
                return DeliveryResult::Failed(format!(
                    "No SockAddrs for ({}, 25)",
                    smtp_server_domain
                ));
            }
        },
    };

    let mailer = match SmtpClient::new(sockaddr, client_security) {
        Ok(m) => m,
        Err(e) => {
            return DeliveryResult::Failed(format!("Unable to setup SMTP transport: {:?}", e));
        }
    };

    // Configure the mailer
    let mut mailer = mailer
        // FIXME, our helo_name is unnecessarily limiting.
        .hello_name( ClientId::Domain(config.helo_name.to_owned()) )
        .smtp_utf8(true) // is only used if the server supports it
        .timeout(Some(Duration::from_secs( config.smtp_timeout_secs )));

    if let Some(ref relay_config) = config.relay_delivery {
        mailer = mailer
            .authentication_mechanism(relay_config.auth.mechanism)
            .credentials(Credentials::new(
                relay_config.auth.username.clone(),
                relay_config.auth.password.clone()
            ));
    }

    let mut mailer = mailer.transport();

    const IGNORED_ATTEMPTS: u8 = 1;

    let result = match mailer.send(sendable_email) {
        Ok(response) => {
            info!("(worker) delivery response: {:?}", response);
            match response.code.severity {
                Severity::PositiveCompletion | Severity::PositiveIntermediate => {
                    DeliveryResult::Delivered(format!("{:?}", response))
                }
                Severity::TransientNegativeCompletion => {
                    DeliveryResult::Deferred(IGNORED_ATTEMPTS, format!("{:?}", response))
                }
                Severity::PermanentNegativeCompletion => {
                    DeliveryResult::Failed(format!("{:?}", response))
                }
            }
        }
        Err(LettreSmtpError::Transient(response)) => {
            info!("(worker) delivery failed response: {:?}", response);
            DeliveryResult::Deferred(IGNORED_ATTEMPTS, format!("{:?}", response))
        }
        Err(LettreSmtpError::Permanent(response)) => {
            info!("(worker) delivery failed response: {:?}", response);
            DeliveryResult::Failed(format!("{:?}", response))
        }
        Err(LettreSmtpError::Resolution) => {
            info!("(worker) delivery failed: DNS resolution failed");
            DeliveryResult::Deferred(IGNORED_ATTEMPTS, "DNS resolution failed".to_owned())
        }
        // FIXME: certain LettreSmtpError::Io errors may also be transient.
        Err(e) => {
            info!("(worker) delivery failed response: {:?}", e);
            DeliveryResult::Failed(format!("{:?}", e))
        }
    };

    mailer.close();

    result
}
