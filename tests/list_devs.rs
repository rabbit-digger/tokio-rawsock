use tokio_rawsock::open_best_library;
use tokio_rawsock::traits::Library;

/*
Tests in this module require correctly setup environment. Therefore they are disabled (ignored)
by default. You can enable them by addding --ignored flag to your cargo testing command.
Some tests also may require administrative privileges.
*/

fn choose_interf(lib: &Box<dyn Library>) -> Option<String> {
    match lib.all_interfaces() {
        Ok(i) => i.first().map(|j| j.name.clone()),
        Err(_) => None,
    }
}

#[test]
#[ignore]
fn list_devs() {
    let lib = open_best_library().expect("Could not open pcap library");

    if let Some(ifname) = choose_interf(&lib) {
        let mut _interf = lib
            .open_interface(&ifname)
            .expect("Could not open interface");
        //on some interfaces there may be no traffic.
    }
}
