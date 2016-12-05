# Mailstrom

[Documentation](https://mikedilger.github.io/mailstrom)

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
   [resolv](https://github.com/mikedilger/resolv-rs) library for DNS lookups (via your
   operating system)
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

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
