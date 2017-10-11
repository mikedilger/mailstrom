NOTE: The newer 0.3.0 version is not on crates.io because it depends on a
      specific git commit of the lettre crate.

# Mailstrom

[Documentation](https://mikedilger.github.io/mailstrom) for 0.3.0

`Mailstrom` is a rust library that handles email delivery for rust programs in a
background worker thread.  It does the following:

 * Accepts an email from the caller and then does everything necessary to get it
   delivered to all recipients without blocking the caller.
 * Allows the caller to query the status of an earlier submitted email at any time,
   to determine if it is Queued, Delivered, Deferred, or has Failed, with details
   as to why, on a per-recipient basis.
 * Handles all parsing, validation, and encoding of email content and headers,
   in compliance with RFC 5322 (and other RFCs).  Uses the
   [email-format](https://github.com/mikedilger/email-format) library for this.
 * Looks up the DNS MX records of the recipients, and delivers directly to those Internet
   mail servers over SMTP, thus not requiring any local SMTP relay.  Uses the
   [trust-dns](https://github.com/bluejekyll/trust-dns) library for DNS lookups
 * SMTP transport "heavy lifting" is performed via the [lettre](https://github.com/lettre/lettre)
   library.  Uses STARTTLS where available.
 * Retries with exponential backoff for a fixed number of retries (currently fixed at 3),
   when the send result is Deferred
 * Uses a pluggable user-defined state management (persistence) layer.

## Limitations

 * Only works on glibc based operating systems, due to usage of the `resolv` library.
 * The [email-format](https://github.com/mikedilger/email-format) crate is somewhat incomplete
   and clunky still.  It doesn't incorporate RFC 6854 (updated From and Sender syntax) yet.
   It defines types one-to-one with ABNF parsing units, rather than as semantic units of meaning.
   And it doesn't let you use obvious types yet like setting the date from a DateTime type.
   However, these issues will be worked out in the near future.
 * This crate is not highly performant. If we wanted high throughput, we should use multiple
   threads and base off of the tokio crate. Maybe in a future version.

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.

# How to avoid having your emails tagged as Spam

Mailstrom does its part to help get your emails delivered, by being compliant with RFC5322
and including a `Message-Id` header in every email.

You are responsible for the lion's share of the work in this regard.  This link at
[Gmail support](https://support.google.com/mail/answer/81126?hl=en&vid=0-289374121815-1481666526430)
is quite helpful. Also,

 * Use a consistent IP address for sending.
 * If possible, have the reverse DNS of your IP address point to the domain name you are
   sending emails from.  For gmail, this is absolutely required when sending over IPv6
 * Use a consistent helo name when sending
 * Use a consistent From email address
 * Publish an SPF TXT record, or better yet, sign email messages with DKIM with a key of at
   least 1024 bits
 * Publish a DMARC policy
 * Don't send spammy content. Don't send phishing content. Subject should be relevant to body.
 * Allow your users to unsubscribe, either by replying or via a link.
   Preferably provide a "List-Unsubscribe" email header pointing to the unsubscribe URL.
 * Automatically unsubscribe users who's address bounces mulitple pieces of mail.
 * If bulk, must have a "Precedence: bulk" header field
 * Separate promotional emails from transactional emails via separate from addresses, or
   even separate IP addresses and sending domains. If your promotional materials become
   classified as spam, at least the transactional emails will still get delivered.
