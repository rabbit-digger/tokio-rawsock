use super::structs::PCapPacketHeader;
use crate::BorrowedPacket;

use libc::c_uchar;
use std::slice::from_raw_parts;
use time::Timespec;

pub fn borrowed_packet_from_header<'a, 'b>(
    header: &'a PCapPacketHeader,
    data: *const u8,
) -> BorrowedPacket<'b> {
    unsafe {
        BorrowedPacket::new(
            Timespec::new(header.ts.tv_sec as i64, (header.ts.tv_usec * 1000) as i32),
            from_raw_parts(data, header.caplen as usize),
        )
    }
}

pub extern "C" fn on_received_packet_static<F>(
    user: *mut c_uchar,
    h: *const PCapPacketHeader,
    bytes: *const c_uchar,
) where
    F: FnMut(&BorrowedPacket),
{
    let callback = unsafe { &mut *(user as *mut F) };

    let packet = borrowed_packet_from_header(unsafe { &*h }, bytes);
    callback(&packet)
}

pub extern "C" fn on_received_packet_dynamic(
    user: *mut c_uchar,
    h: *const PCapPacketHeader,
    bytes: *const c_uchar,
) {
    let callback = unsafe { &mut *(user as *mut &mut dyn FnMut(&BorrowedPacket)) };

    let packet = borrowed_packet_from_header(unsafe { &*h }, bytes);
    callback(&packet)
}
