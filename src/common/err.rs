use dlopen::Error as DlopenError;
use std::ffi::NulError;
use thiserror::Error;

/// Error enumeration returned by this crate.
#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    DllError(#[from] DlopenError),
    ///Provided string could not be coverted into `std::ffi::CString` because it contained null
    /// character.
    #[error("{0}")]
    NullCharacter(#[from] NulError),
    ///The interface could not be opened.
    #[error("{0}")]
    OpeningInterface(String),
    ///Receiving raw packet failed.
    #[error("{0}")]
    ReceivingPacket(String),
    ///Sending raw packet failed.
    #[error("{0}")]
    SendingPacket(String),
    ///Obtaining device description list failed.
    #[error("{0}")]
    GettingDeviceDescriptionList(String),
    ///No paths were provided by the user
    #[error("No library paths were provided.")]
    NoPathsProvided,
    #[error("{0}")]
    LibraryError(String),
}
