use super::dll::helpers::PCapErrBuf;
use super::dll::{PCapHandle, PCapPacketHeader, PCapSendQueue, WPCapDll};
use super::structs::PCapStat;
use crate::pcap_common::constants::{PCAP_EMPTY_FILTER_STR, PCAP_ERROR_BREAK, SUCCESS};
use crate::pcap_common::helpers::{
    borrowed_packet_from_header, on_received_packet_dynamic, on_received_packet_static,
};
use crate::pcap_common::BpfProgram;
use crate::utils::cstr_to_string;
use crate::{traits, BorrowedPacket, DataLink, Error, Stats};
use libc::{c_int, c_uint};
use std::ffi::{CStr, CString};
use std::mem::{transmute, zeroed, MaybeUninit};

const QUEUE_SIZE: usize = 65536 * 8; //min 8 packets

///wpcap specific Interface representation.
pub struct Interface<'a> {
    handle: *const PCapHandle,
    dll: &'a WPCapDll,
    datalink: DataLink,
    queue: *mut PCapSendQueue,
}

unsafe impl<'a> Sync for Interface<'a> {}
unsafe impl<'a> Send for Interface<'a> {}

impl<'a> Interface<'a> {
    pub fn new(name: &str, dll: &'a WPCapDll) -> Result<Self, Error> {
        let name = CString::new(name)?;
        let mut errbuf = PCapErrBuf::new();
        let handle = unsafe {
            dll.pcap_open_live(
                name.as_ptr(),
                65536, /* max packet size */
                8,     /* promiscuous mode */
                1000,  /* read timeout in milliseconds */
                errbuf.buffer(),
            )
        };
        if handle.is_null() {
            return Err(Error::OpeningInterface(errbuf.as_string()));
        }
        let queue = unsafe { dll.pcap_sendqueue_alloc(QUEUE_SIZE as c_uint) };
        assert!(!queue.is_null());
        let datalink = match unsafe { dll.pcap_datalink(handle) } {
            1 => DataLink::Ethernet,
            12 => DataLink::RawIp,
            _ => DataLink::Other,
        };

        Ok(Interface {
            handle,
            dll,
            datalink,
            queue,
        })
    }

    fn last_error(&self) -> Error {
        let cerr = unsafe { self.dll.pcap_geterr(self.handle) };
        Error::LibraryError(cstr_to_string(cerr))
    }
}

impl<'a> Drop for Interface<'a> {
    fn drop(&mut self) {
        unsafe {
            self.dll.pcap_sendqueue_destroy(self.queue);
            self.dll.pcap_close(self.handle);
        }
    }
}

impl<'a> traits::DynamicInterface<'a> for Interface<'a> {
    fn send(&self, packet: &[u8]) -> Result<(), Error> {
        let header = PCapPacketHeader {
            len: packet.len() as c_uint,
            caplen: packet.len() as c_uint,
            ts: unsafe { zeroed() },
            #[cfg(any(target_os = "macos", target_os = "ios"))]
            comment: unsafe { zeroed() },
        };

        let err = unsafe {
            self.dll
                .pcap_sendqueue_queue(self.queue, &header, packet.as_ptr())
        };
        if err != 0 {
            self.flush();
            let err = unsafe {
                self.dll
                    .pcap_sendqueue_queue(self.queue, &header, packet.as_ptr())
            };
            assert_eq!(err, 0);
        }

        if unsafe {
            self.dll
                .pcap_sendpacket(self.handle, packet.as_ptr(), packet.len() as c_int)
        } == SUCCESS
        {
            Ok(())
        } else {
            let txt = unsafe { CStr::from_ptr(self.dll.pcap_geterr(self.handle)) }
                .to_string_lossy()
                .into_owned();
            Err(Error::SendingPacket(txt))
        }
    }

    fn receive<'b>(&mut self) -> Result<BorrowedPacket, Error> {
        let mut header = MaybeUninit::<PCapPacketHeader>::uninit();
        //TODO: replace pcap_next with pcap_next_ex to obtain more error information
        let data = unsafe { self.dll.pcap_next(self.handle, header.as_mut_ptr()) };
        if data.is_null() {
            Err(Error::ReceivingPacket(
                "Unknown error when obtaining packet".into(),
            ))
        } else {
            Ok(borrowed_packet_from_header(
                &unsafe { header.assume_init() },
                data,
            ))
        }
    }

    fn flush(&self) {
        unsafe {
            self.dll.pcap_sendqueue_transmit(self.handle, self.queue, 0);
            /*
            Those calls are reported by masscan code to be necessary
            although I can't find any reason for that. For now disabled.
            self.dll.pcap_sendqueue_destroy(self.queue);
            self.queue = self.dll.pcap_sendqueue_alloc(QUEUE_SIZE as c_uint);
            */
        }
    }

    fn data_link(&self) -> DataLink {
        self.datalink
    }

    fn stats(&self) -> Result<Stats, Error> {
        let mut stats = MaybeUninit::<PCapStat>::uninit();
        if SUCCESS == unsafe { self.dll.pcap_stats(self.handle, stats.as_mut_ptr()) } {
            let stats = unsafe { stats.assume_init() };
            Ok(Stats {
                received: stats.ps_recv as u64,
                dropped: stats.ps_drop as u64, //sp_ifdrop is not yet supported.
            })
        } else {
            Err(self.last_error())
        }
    }

    fn break_loop(&self) {
        unsafe { self.dll.pcap_breakloop(self.handle) }
    }

    fn loop_infinite_dyn(&self, callback: &dyn FnMut(&BorrowedPacket)) -> Result<(), Error> {
        let result = unsafe {
            self.dll.pcap_loop(
                self.handle,
                -1,
                on_received_packet_dynamic,
                transmute(&callback),
            )
        };
        if result == SUCCESS || result == PCAP_ERROR_BREAK {
            Ok(())
        } else {
            Err(self.last_error())
        }
    }

    fn set_filter_cstr(&mut self, filter: &CStr) -> Result<(), Error> {
        let mut bpf_filter = MaybeUninit::<BpfProgram>::uninit();
        let result = unsafe {
            self.dll
                .pcap_compile(self.handle, bpf_filter.as_mut_ptr(), filter.as_ptr(), 1, 0)
        };
        if result != SUCCESS {
            return Err(self.last_error());
        }

        let result = unsafe {
            self.dll
                .pcap_setfilter(self.handle, bpf_filter.as_mut_ptr())
        };
        unsafe { self.dll.pcap_freecode(bpf_filter.as_mut_ptr()) };
        if result == SUCCESS {
            Ok(())
        } else {
            Err(self.last_error())
        }
    }

    fn remove_filter(&mut self) -> Result<(), Error> {
        self.set_filter_cstr(unsafe { CStr::from_bytes_with_nul_unchecked(PCAP_EMPTY_FILTER_STR) })
    }
}

impl<'a> traits::StaticInterface<'a> for Interface<'a> {
    fn loop_infinite<F>(&self, callback: F) -> Result<(), Error>
    where
        F: FnMut(&BorrowedPacket),
    {
        let result = unsafe {
            self.dll.pcap_loop(
                self.handle,
                -1,
                on_received_packet_static::<F>,
                transmute(&callback),
            )
        };
        if result == SUCCESS || result == PCAP_ERROR_BREAK {
            Ok(())
        } else {
            Err(self.last_error())
        }
    }
}
