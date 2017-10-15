
use trust_dns_resolver::Resolver;
use internal_message_status::InternalMessageStatus;
use message_status::DeliveryResult;

// Get MX records for email recipients
pub fn get_mx_records_for_email(internal_message_status: &mut InternalMessageStatus,
                                resolver: &Resolver)
{
    use std::net::{SocketAddr, ToSocketAddrs};

    // Look-up the MX records for each recipient
    for recipient in &mut internal_message_status.recipients {
        let mx_record_strings = get_mx_records_for_domain(&*recipient.domain, resolver);
        let mut mx_record_sockaddrs: Vec<SocketAddr> = Vec::new();
        for record in mx_record_strings {
            match (&*record, 25_u16).to_socket_addrs() {
                Err(_) => {
                    warn!("ToSocketAddr FAILED FOR {}: {}",
                          recipient.email_addr,
                          &*record);
                    continue; // MX record invalid?
                },
                Ok(mut iter) => match iter.next() {
                    Some(sa) => mx_record_sockaddrs.push(sa),
                    None => continue, // No MX records
                }
            }
        }
        if mx_record_sockaddrs.len() == 0 {
            recipient.result = DeliveryResult::Failed(
                "MX records found but none are valid".to_owned());
            continue;
        }

        recipient.mx_servers = Some(mx_record_sockaddrs);
        debug!("DEBUG: got mx servers for {}: {:?}",
               recipient.email_addr,
               recipient.mx_servers.as_ref().unwrap());
    }
}

// Get MX records for a domain, in order of preference
fn get_mx_records_for_domain(domain: &str, resolver: &Resolver)
                             -> Vec<String>
{
    use std::cmp::Ordering;

    let response = match resolver.mx_lookup(domain) {
        Ok(res) => res,
        Err(_) => {
            // fallback to the domain (RFC 5321)
            return vec![domain.to_owned()];
        }
    };

    let mut records: Vec<(u16,String)> = response.iter()
        .map(|mx| (mx.preference(), mx.exchange().to_string()))
        .collect();

    if records.len() == 0 {
        // fallback to the domain (RFC 5321)
        return vec![domain.to_owned()];
    }

    // Sort by priority
    records.sort_by(|a,b| a.0.cmp(&b.0));

    // Move any results that end in a digit to the end (domain names are preferred
    // over IP addresses, regardless of their MX setting, due to the inability to
    // verify certificates with IP addresses)
    records.sort_by(|a,b| {
        let a_is_ip = if let Some(last) = a.1.chars().rev().next() {
            if last.is_digit(10) { true }
            else { false }
        } else { false };

        let b_is_ip = if let Some(last) = b.1.chars().rev().next() {
            if last.is_digit(10) { true }
            else { false }
        } else { false };

        match (a_is_ip, b_is_ip) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            _ => Ordering::Equal
        }
    });

    records.into_iter().map(|(_,exch)| exch).collect()
}
