use super::is_ip;
use message_status::InternalMessageStatus;
use trust_dns_resolver::Resolver;

// Get MX records for email recipients
pub fn get_mx_records_for_email(
    internal_message_status: &mut InternalMessageStatus,
    resolver: &Resolver,
) {
    // Look-up the MX records for each recipient
    for recipient in &mut internal_message_status.recipients {
        let mx_records = get_mx_records_for_domain(&*recipient.domain, resolver);
        recipient.mx_servers = Some(mx_records);
        debug!(
            "DEBUG: got mx servers for {}: {:?}",
            recipient.email_addr,
            recipient.mx_servers.as_ref().unwrap()
        );
    }
}

// Get MX records for a domain, in order of preference
fn get_mx_records_for_domain(domain: &str, resolver: &Resolver) -> Vec<String> {
    use std::cmp::Ordering;

    let response = match resolver.mx_lookup(domain) {
        Ok(res) => res,
        Err(_) => {
            // fallback to the domain (RFC 5321)
            return vec![domain.to_owned()];
        }
    };

    let mut records: Vec<(u16, String)> = response
        .iter()
        .map(|mx| (mx.preference(), mx.exchange().to_string()))
        .collect();

    if records.is_empty() {
        // fallback to the domain (RFC 5321)
        return vec![domain.to_owned()];
    }

    // Sort by priority
    records.sort_by(|a, b| a.0.cmp(&b.0));

    // Move any results that end in a digit to the end (domain names are preferred
    // over IP addresses, regardless of their MX setting, due to the inability to
    // verify certificates with IP addresses)
    records.sort_by(|a, b| {
        let a_is_ip = is_ip(&*(a.1));
        let b_is_ip = is_ip(&*(b.1));
        match (a_is_ip, b_is_ip) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => Ordering::Equal,
        }
    });

    records
        .into_iter()
        .map(|(_, exch)| exch.trim_end_matches(|c| c == '.').to_owned())
        .collect()
}
