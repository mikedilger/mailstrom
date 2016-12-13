
extern crate env_logger;

use {Mailstrom, WorkerStatus, Config};
use storage::MemoryStorage;


#[test]
fn test_terminate() {
    let mut mailstrom = Mailstrom::new(
        Config {
            helo_name: "localhost".to_owned(),
            smtp_timeout_secs: 30,
        },
        MemoryStorage::new());

    assert_eq!( mailstrom.worker_status(), WorkerStatus::Ok );
    mailstrom.die().unwrap();
    ::std::thread::sleep(::std::time::Duration::from_millis(100));
    assert_eq!( mailstrom.worker_status(), WorkerStatus::Terminated );
}
