
extern crate env_logger;

use Mailstrom;

#[test]
fn test_terminate() {
    let mut mailstrom = Mailstrom::new();
    assert!( !mailstrom.is_dead() );
    mailstrom.die().unwrap();
    ::std::thread::sleep(::std::time::Duration::from_millis(100));
    assert!( mailstrom.is_dead() );
}
