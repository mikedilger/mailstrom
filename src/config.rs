use trust_dns_resolver::config::{ResolverConfig, ResolverOpts};

#[derive(Clone)]
pub struct Config {
    pub helo_name: String,
    pub smtp_timeout_secs: u64,
    pub base_resend_delay_secs: u64,
    pub resolver_config: ResolverConfig,
    pub resolver_opts: ResolverOpts,
    pub require_tls: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            helo_name: "localhost".to_string(),
            smtp_timeout_secs: 60,
            base_resend_delay_secs: 60,
            resolver_config: ResolverConfig::default(),
            resolver_opts: ResolverOpts::default(),
            require_tls: true,
        }
    }
}
