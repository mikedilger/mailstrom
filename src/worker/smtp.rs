
use std::net::ToSocketAddrs;
use std::time::Duration;
use native_tls::TlsConnector;
use lettre::smtp::{SmtpTransportBuilder, ClientSecurity};
use lettre::smtp::response::Severity;
use lettre::smtp::error::Error as LettreSmtpError;
use lettre::EmailTransport;
use lettre::smtp::client::net::{ClientTlsParameters, DEFAULT_TLS_PROTOCOLS};
use lettre::smtp::extension::ClientId;
use prepared_email::PreparedEmail;
use delivery_result::DeliveryResult;
use Config;

// Deliver an email to an SMTP server
pub fn smtp_delivery<'a>(prepared_email: &PreparedEmail,
                         smtp_server_domain: &str,
                         config: &Config,
                         attempt: u8)
                         -> DeliveryResult
{
    trace!("SMTP delivery to [{}] at {}",
           prepared_email.to.join(", "), smtp_server_domain);

    let smtp_server_sockaddr = match (smtp_server_domain, 25_u16).to_socket_addrs() {
        Err(e) => {
            warn!("ToSocketAddr failed for ({}, 25): {:?}", smtp_server_domain, e);
            return DeliveryResult::Failed(
                format!("ToSockaddr failed for ({}, 25): {:?}", smtp_server_domain, e) )
        },
        Ok(mut iter) => match iter.next() {
            Some(sa) => sa,
            None => {
                warn!("No SockAddrs for ({}, 25)", smtp_server_domain);
                return DeliveryResult::Failed(
                    format!("No SockAddrs for ({}, 25)", smtp_server_domain) )
            }
        }
    };

    let mut tls_builder = match TlsConnector::builder() {
        Ok(builder) => builder,
        Err(e) => {
            info!("(worker) failed to create TLS Connector: {:?}", e);
            return DeliveryResult::Failed( format!("Failed to create TLS connector: {:?}", e) )
        }
    };

    if let Err(e) = tls_builder.supported_protocols(DEFAULT_TLS_PROTOCOLS) {
        info!("(worker) failed to set default tls protocols: {:?}", e);
        return DeliveryResult::Failed( format!("Failed to set supported protocols: {:?}", e) )
    }

    let tls_parameters = ClientTlsParameters::new(
        smtp_server_domain.to_owned(),
        tls_builder.build().unwrap(),
    );

    let client_security = match config.require_tls {
        true => ClientSecurity::Required(tls_parameters),
        false => ClientSecurity::Opportunistic(tls_parameters),
    };

    let mailer = match SmtpTransportBuilder::new(smtp_server_sockaddr, client_security) {
        Ok(m) => m,
        Err(e) => {
            return DeliveryResult::Failed(
                format!("Unable to setup SMTP transport: {:?}", e));
        }
    };

    // Configure the mailer
    let mut mailer = mailer
        // FIXME, our config.helo_name is unnecessarily limiting.
        .hello_name( ClientId::Domain(config.helo_name.clone()) )
        .smtp_utf8(true) // is only used if the server supports it
        .timeout(Some(Duration::from_secs( config.smtp_timeout_secs )))
        .build();

    let result = match mailer.send(prepared_email) {
        Ok(response) => {
            info!("(worker) delivery response: {:?}", response);
            match response.code.severity {
                Severity::PositiveCompletion | Severity::PositiveIntermediate => {
                    DeliveryResult::Delivered( format!("{:?}", response) )
                },
                Severity::TransientNegativeCompletion => {
                    DeliveryResult::Deferred( attempt, format!("{:?}", response) )
                },
                Severity::PermanentNegativeCompletion => {
                    DeliveryResult::Failed( format!("{:?}", response) )
                },
            }
        },
        Err(LettreSmtpError::Transient(response)) => {
            info!("(worker) delivery failed response: {:?}", response);
            DeliveryResult::Deferred( attempt, format!("{:?}", response) )
        },
        Err(LettreSmtpError::Permanent(response)) => {
            info!("(worker) delivery failed response: {:?}", response);
            DeliveryResult::Failed( format!("{:?}", response) )
        },
        Err(LettreSmtpError::Resolution) => {
            info!("(worker) delivery failed: DNS resolution failed");
            DeliveryResult::Deferred( attempt, "DNS resolution failed".to_owned() )
        },
        // FIXME: certain LettreSmtpError::Io errors may also be transient.
        Err(e) => {
            info!("(worker) delivery failed response: {:?}", e);
            DeliveryResult::Failed( format!("{:?}", e) )
        },
    };

    mailer.close();

    result
}
