
extern crate env_logger;

use email_format::Email;
use {Mailstrom, WorkerStatus, Config};
use storage::MemoryStorage;


#[test]
fn test_terminate() {
    let mut mailstrom = Mailstrom::new(Config { helo_name: "localhost".to_owned() },
                                       MemoryStorage::new());
    assert_eq!( mailstrom.worker_status(), WorkerStatus::Ok );
    mailstrom.die().unwrap();
    ::std::thread::sleep(::std::time::Duration::from_millis(100));
    assert_eq!( mailstrom.worker_status(), WorkerStatus::Terminated );
}

#[test]
fn test_submit_email() {
    let mut mailstrom = Mailstrom::new(Config { helo_name: "localhost".to_owned() },
                                       MemoryStorage::new());

    let mut email = Email::new(
        "myself@mydomain.com",  // "From:"
        "Wed, 05 Jan 2015 15:13:05 +1300" // "Date:"
            ).unwrap();
    email.set_bcc("myself@mydomain.com").unwrap();
    email.set_sender("from_myself@mydomain.com").unwrap();
    email.set_reply_to("My Mailer <no-reply@mydomain.com>").unwrap();
    email.set_to("You <you@yourdomain.com>, AndYou <andyou@yourdomain.com>").unwrap();
    email.set_cc("Our Friend <friend@frienddomain.com>").unwrap();
    email.set_subject("Hello Friend").unwrap();
    email.set_body("Good to hear from you.\r\n\
                    I wish you the best.\r\n\
                    \r\n\
                    Your Friend").unwrap();

    assert!( mailstrom.send_email(email).is_ok() );
}
