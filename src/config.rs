pub use lettre::smtp::authentication::Mechanism;
pub use trust_dns_resolver::config::{ResolverConfig, ResolverOpts, NameServerConfig, Protocol};

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
    pub auth: SmtpAuth,
}

/// Delivery configuration needed if delivering directly to MX servers
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
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

/// Mailstrom configuration settings
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub helo_name: String,
    pub smtp_timeout_secs: u64,
    pub base_resend_delay_secs: u64,
    pub require_tls: bool,
    pub relay_delivery: Option<RelayConfig>,
    pub remote_delivery: Option<RemoteDeliveryConfig>,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            helo_name: "localhost".to_string(),
            smtp_timeout_secs: 60,
            base_resend_delay_secs: 60,
            require_tls: false,
            relay_delivery: None,
            remote_delivery: Some(Default::default()),
        }
    }
}

impl Config {
    pub fn is_valid(&self) -> bool {
        // Exactly one of these must be true, the other false:
        self.relay_delivery.is_some() ^ self.remote_delivery.is_some()
    }
}
