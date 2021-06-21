/*!
Code for managing the *wpcap* library (Windows extension to pcap).
*/

pub mod dll;
mod interface;
mod library;
mod paths;
mod structs;

pub use self::interface::Interface;
pub use self::library::Library;
pub use self::paths::DEFAULT_PATHS;
