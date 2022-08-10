use crate::config::{Config, DeliveryConfig};
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
use std::io::ErrorKind;

// Deliver an email to an SMTP server
pub fn smtp_delivery(
    prepared_email: &PreparedEmail,
    smtp_server_domain: &str,
    port: u16,
    config: &Config
) -> DeliveryResult {

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

    let client_security = if let DeliveryConfig::Relay(ref rc) = config.delivery {
        if rc.use_tls {
            let tls_parameters =
                ClientTlsParameters::new(smtp_server_domain.to_owned(), tls_builder);
            if config.require_tls {
                ClientSecurity::Required(tls_parameters)
            } else {
                ClientSecurity::Opportunistic(tls_parameters)
            }
        } else {
            ClientSecurity::None
        }
    } else {
        let tls_parameters =
            ClientTlsParameters::new(smtp_server_domain.to_owned(), tls_builder);

        if config.require_tls {
            ClientSecurity::Required(tls_parameters)
        } else {
            ClientSecurity::Opportunistic(tls_parameters)
        }
    };

    // Build sockaddr
    let sockaddr = match (smtp_server_domain, port).to_socket_addrs() {
        Err(e) => {
            warn!(
                "ToSocketAddr failed for ({}, {}): {:?}",
                smtp_server_domain, port, e
            );
            return DeliveryResult::Failed(format!(
                "ToSockaddr failed for ({}, {}): {:?}",
                smtp_server_domain, port, e
            ));
        }
        Ok(mut iter) => match iter.next() {
            Some(sa) => sa,
            None => {
                warn!("No SockAddrs for ({}, {})", smtp_server_domain, port);
                return DeliveryResult::Failed(format!(
                    "No SockAddrs for ({}, {})",
                    smtp_server_domain, port
                ));
            }
        },
    };

    let mailer = match SmtpClient::new(sockaddr, client_security) {
        Ok(m) => m,
        Err(e) => {
            info!("(worker) failed to setup SMTP transport: {:?}", e);
            return DeliveryResult::Failed(format!("Unable to setup SMTP transport: {:?}", e));
        }
    };

    // Configure the mailer
    let mut mailer = mailer
        // FIXME, our helo_name is unnecessarily limiting.
        .hello_name( ClientId::Domain(config.helo_name.to_owned()) )
        .smtp_utf8(true) // is only used if the server supports it
        .timeout(Some(Duration::from_secs( config.smtp_timeout_secs )));

    if let DeliveryConfig::Relay(ref relay_config) = config.delivery {
        if let Some(ref auth) = relay_config.auth {
            mailer = mailer
                .authentication_mechanism(auth.mechanism)
                .credentials(Credentials::new(
                    auth.username.clone(),
                    auth.password.clone()
                ));
        }
    }

    let mut mailer = mailer.transport();

    const IGNORED_ATTEMPTS: u8 = 1;

    debug!(
        "Starting SMTP delivery to [{}] at {}",
        prepared_email.to.join(", "),
        smtp_server_domain
    );

    #[allow(unreachable_patterns)] // lettre may add more
    let result = match mailer.send(sendable_email) {
        Ok(response) => {
            match response.code.severity {
                Severity::PositiveCompletion | Severity::PositiveIntermediate => {
                    info!("(worker) Delivery Success: {:?}", response);
                    DeliveryResult::Delivered(format!("{:?}", response))
                }
                Severity::TransientNegativeCompletion => {
                    info!("(worker) Delivery Deferred: {:?}", response);
                    DeliveryResult::Deferred(IGNORED_ATTEMPTS, format!("{:?}", response))
                }
                Severity::PermanentNegativeCompletion => {
                    info!("(worker) Delivery Failed: {:?}", response);
                    DeliveryResult::Failed(format!("{:?}", response))
                }
            }
        },
        Err(LettreSmtpError::Transient(response)) => {
            info!("(worker) Delivery Deferred: {:?}", response);
            DeliveryResult::Deferred(IGNORED_ATTEMPTS, format!("{:?}", response))
        },
        Err(LettreSmtpError::Permanent(response)) => {
            info!("(worker) Delivery Failed: {:?}", response);
            DeliveryResult::Failed(format!("{:?}", response))
        },
        Err(LettreSmtpError::Resolution) => {
            info!("(worker) DNS resolution failed");
            DeliveryResult::Deferred(IGNORED_ATTEMPTS, "DNS resolution failed".to_owned())
        },
        Err(LettreSmtpError::ResponseParsing(s)) => {
            info!("(worker) Delivery Failed (response parsing error): {}", s);
            DeliveryResult::Failed(format!("response parsing error: {}", s))
        },
        Err(LettreSmtpError::ChallengeParsing(de)) => {
            info!("(worker) Delivery Failed (challenge parsing error): {:?}", de);
            DeliveryResult::Failed(format!("challenge parsing error: {:?}", de))
        },
        Err(LettreSmtpError::Utf8Parsing(fue)) => {
            info!("(worker) Delivery Failed (utf8 parsing error): {:?}", fue);
            DeliveryResult::Failed(format!("utf8 parsing error: {:?}", fue))
        },
        Err(LettreSmtpError::Client(s)) => {
            info!("(worker) Delivery Failed (internal client error): {}", s);
            DeliveryResult::Failed(format!("internal client error: {:?}", s))
        },
        Err(LettreSmtpError::Io(ioe)) => {
            match ioe.kind() {
                ErrorKind::ConnectionRefused |
                ErrorKind::ConnectionReset |
                // The following are only available on nightly (see rust #86442)
                // ErrorKind::HostUnreachable |
                // ErrorKind::NetworkUnreachable |
                // ErrorKind::NetworkDown |
                // ErrorKind::ResourceBusy |
                ErrorKind::ConnectionAborted |
                ErrorKind::AddrInUse |
                ErrorKind::BrokenPipe |
                ErrorKind::TimedOut |
                ErrorKind::Interrupted => {
                    info!("(worker) Delivery Deferred (I/O error): {:?}", ioe);
                    DeliveryResult::Deferred(IGNORED_ATTEMPTS, format!("I/O error: {:?}", ioe))
                },
                _ => {
                    // We still might defer on other errors that stable rust doesn't
                    // represent as enum variants in std::io::ErrorKind yet. We find
                    // these by inspecting their debug representations
                    let asdebug = format!("{:?}", ioe);
                    if asdebug.contains("kind: HostUnreachable") ||
                        asdebug.contains("kind: NetworkUnreachable") ||
                        asdebug.contains("kind: NetworkDown") ||
                        asdebug.contains("kind: ResourceBusy")
                    {
                        info!("(worker) Delivery Deferred (I/O error): {:?}", ioe);
                        DeliveryResult::Deferred(IGNORED_ATTEMPTS, format!("I/O error: {:?}", ioe))
                    } else {
                        info!("(worker) Delivery Failed (I/O error): {:?}", ioe);
                        DeliveryResult::Failed(format!("I/O error: {:?}", ioe))
                    }
                }
            }
        },
        Err(LettreSmtpError::Tls(tlse)) => {
            info!("(worker) Delivery Failed (TLS error): {:?}", tlse);
            DeliveryResult::Failed(format!("TLS error: {:?}", tlse))
        },
        Err(LettreSmtpError::Parsing(nomek)) => {
            info!("(worker) Delivery Failed (Parsing error): {:?}", nomek);
            DeliveryResult::Failed(format!("Parsing error: {:?}", nomek))
        },
        Err(e) => {
            info!("(worker) delivery failed response: {:?}", e);
            DeliveryResult::Failed(format!("{:?}", e))
        }
    };

    mailer.close();

    result
}
