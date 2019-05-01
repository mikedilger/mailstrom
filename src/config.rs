pub use lettre::smtp::authentication::Mechanism;
pub use trust_dns_resolver::config::{ResolverConfig, ResolverOpts, NameServerConfig, Protocol};
use std::net::SocketAddr;

/// Authentication settings for an SMTP relay
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SmtpAuth {
    pub mechanism: Mechanism,
    pub username: String,
    pub password: String,
}

/// Delivery configuration needed if using an SMTP relay
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RelayConfig {
    pub domain_name: String,
    pub port: Option<u16>,
    pub use_tls: bool,
    pub auth: SmtpAuth,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ResolverSetup {
    SystemConf,
    Google,
    Cloudflare,
    Quad9,
    Specific {
        socket: SocketAddr,
        protocol: Protocol,
        tls_dns_name: Option<String>
    }
}

impl Default for ResolverSetup {
    fn default() -> ResolverSetup {
        ResolverSetup::SystemConf
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct RemoteDeliveryConfig {
    pub resolver_setup: ResolverSetup
}

impl Default for RemoteDeliveryConfig {
    fn default() -> RemoteDeliveryConfig {
        RemoteDeliveryConfig {
            resolver_setup: Default::default()
        }
    }
}

/// Delivery configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum DeliveryConfig {
    /// Deliver everything through an SMTP relay
    Relay(RelayConfig),
    /// Deliver directly directly to recipient domain MX servers
    Remote(RemoteDeliveryConfig)
}

impl Default for DeliveryConfig {
    fn default() -> DeliveryConfig {
        DeliveryConfig::Remote(RemoteDeliveryConfig::default())
    }
}

/// Mailstrom configuration settings
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub helo_name: String,
    pub smtp_timeout_secs: u64,
    pub base_resend_delay_secs: u64,
    pub require_tls: bool,
    pub delivery: DeliveryConfig,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            helo_name: "localhost".to_string(),
            smtp_timeout_secs: 60,
            base_resend_delay_secs: 60,
            require_tls: false,
            delivery: Default::default(),
        }
    }
}
