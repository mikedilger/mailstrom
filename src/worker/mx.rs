
use internal_status::InternalStatus;
use status::DeliveryResult;
use error::Error;

// Get MX records for email recipients
pub fn get_mx_records_for_email(internal_status: &mut InternalStatus)
{
    use std::net::{SocketAddr, ToSocketAddrs};

    // Look-up the MX records for each recipient
    for recipient in &mut internal_status.recipients {
        let mx_records = match get_mx_records_for_domain(&*recipient.domain) {
            Err(e) => {
                recipient.result = DeliveryResult::Failed(
                    format!("Unable to fetch MX record: {:?}", e));
                warn!("MX LOOKUP FAILED FOR {}", recipient.email_addr);
                continue;
            }
            Ok(records) => {
                let mut mx_records: Vec<SocketAddr> = Vec::new();
                for record in records {
                    match (&*record, 25_u16).to_socket_addrs() {
                        Err(_) => {
                            warn!("ToSocketAddr FAILED FOR {}: {}",
                                  recipient.email_addr,
                                     &*record);
                            continue; // MX record invalid?
                        },
                        Ok(mut iter) => match iter.next() {
                            Some(sa) => mx_records.push(sa),
                            None => continue, // No MX records
                        }
                    }
                }
                if mx_records.len() == 0 {
                    recipient.result = DeliveryResult::Failed(
                        "MX records found but none are valid".to_owned());
                    continue;
                }
                mx_records
            }
        };
        recipient.mx_servers = Some(mx_records);
        debug!("DEBUG: got mx servers for {}: {:?}",
               recipient.email_addr,
               recipient.mx_servers.as_ref().unwrap());
    }
}

// Get MX records for a domain, in order of preference
fn get_mx_records_for_domain(domain: &str) -> Result<Vec<String>, Error>
{
    use trust_dns_resolver::Resolver;
    use trust_dns_resolver::config::*;

    // FIXME:  make resolver global, so we don't keep recreating it.
    let resolver = Resolver::new(
        ResolverConfig::default(),
        ResolverOpts::default())?;

    let response = resolver.mx_lookup(domain)?;

    let mut records: Vec<(u16,String)> = response.iter()
        .map(|mx| (mx.preference(), mx.exchange().to_string()))
        .collect();
    records.sort_by(|a,b| a.0.cmp(&b.0));
    Ok( records.into_iter().map(|(_,exch)| exch).collect() )
}
