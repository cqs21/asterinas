// SPDX-License-Identifier: MPL-2.0

use core::sync::atomic::{AtomicU32, Ordering};

use aster_util::printer::VmPrinter;

use crate::{
    fs::{
        file::mkmod,
        procfs::template::{FileOps, ProcFileBuilder, read_i32_from},
        vfs::inode::Inode,
    },
    prelude::*,
};

/// Represents the inode at `/proc/sys/fs/lease-break-time`.
pub struct LeaseBreakTimeFileOps;

// Linux defaults to 45 seconds for lease break timeout.
static LEASE_BREAK_TIME_SECONDS: AtomicU32 = AtomicU32::new(45);

impl LeaseBreakTimeFileOps {
    pub fn new_inode(parent: Weak<dyn Inode>) -> Arc<dyn Inode> {
        // Reference:
        // <https://docs.kernel.org/admin-guide/sysctl/fs.html#lease-break-time>
        ProcFileBuilder::new(Self, mkmod!(a+r, u+w))
            .parent(parent)
            .build()
            .unwrap()
    }
}

impl FileOps for LeaseBreakTimeFileOps {
    fn read_at(&self, offset: usize, writer: &mut VmWriter) -> Result<usize> {
        let mut printer = VmPrinter::new_skip(writer, offset);
        writeln!(
            printer,
            "{}",
            LEASE_BREAK_TIME_SECONDS.load(Ordering::Relaxed)
        )?;

        Ok(printer.bytes_written())
    }

    fn write_at(&self, _offset: usize, reader: &mut VmReader) -> Result<usize> {
        let (val, read_bytes) = read_i32_from(reader)?;
        let seconds = u32::try_from(val).map_err(|_| {
            Error::with_message(
                Errno::EINVAL,
                "lease-break-time must be a non-negative integer",
            )
        })?;

        LEASE_BREAK_TIME_SECONDS.store(seconds, Ordering::Relaxed);
        Ok(read_bytes)
    }
}
