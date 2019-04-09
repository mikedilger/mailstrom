extern crate env_logger;

use crate::config::Config;
use crate::storage::MemoryStorage;
use crate::worker::WorkerStatus;
use crate::Mailstrom;

#[test]
fn test_terminate() {
    let mut mailstrom = Mailstrom::new(Config::default(), MemoryStorage::new())
        .unwrap();

    assert_eq!(mailstrom.worker_status(), WorkerStatus::Ok);
    mailstrom.die().unwrap();
    ::std::thread::sleep(::std::time::Duration::from_millis(100));
    assert_eq!(mailstrom.worker_status(), WorkerStatus::Terminated);
}
