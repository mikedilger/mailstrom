use lettre::smtp::authentication::Mechanism;
use trust_dns_resolver::config::{ResolverConfig, ResolverOpts};

/// Authentication settings for an SMTP relay
#[derive(Clone)]
pub struct SmtpAuth {
    pub mechanism: Mechanism,
    pub username: String,
    pub password: String,
}

/// Delivery configuration needed if using an SMTP relay
#[derive(Clone)]
pub struct RelayConfig {
    pub domain_name: String,
    pub auth: SmtpAuth,
}

/// Delivery configuration needed if delivering directly to MX servers
#[derive(Clone)]
pub struct RemoteDeliveryConfig {
    pub resolver_config: ResolverConfig,
    pub resolver_opts: ResolverOpts,
}

impl Default for RemoteDeliveryConfig {
    fn default() -> RemoteDeliveryConfig {
        RemoteDeliveryConfig {
            resolver_config: ResolverConfig::default(),
            resolver_opts: ResolverOpts::default(),
        }
    }
}

/// Delivery configuration
#[derive(Clone)]
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
#[derive(Clone)]
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
            delivery: DeliveryConfig::default(),
        }
    }
}
