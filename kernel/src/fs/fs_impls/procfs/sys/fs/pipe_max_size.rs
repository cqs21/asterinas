// SPDX-License-Identifier: MPL-2.0

use aster_util::printer::VmPrinter;

use crate::{
    fs::{
        file::mkmod,
        procfs::template::{FileOps, ProcFileBuilder},
        vfs::inode::Inode,
    },
    prelude::*,
};

/// Represents the inode at `/proc/sys/fs/pipe-max-size`.
pub struct PipeMaxSizeFileOps;

impl PipeMaxSizeFileOps {
    pub fn new_inode(parent: Weak<dyn Inode>) -> Arc<dyn Inode> {
        // Reference:
        // <https://man7.org/linux/man-pages/man7/pipe.7.html>
        // <https://docs.kernel.org/admin-guide/sysctl/fs.html#pipe-user-pages-hard>
        ProcFileBuilder::new(Self, mkmod!(a+r))
            .parent(parent)
            .build()
            .unwrap()
    }
}

impl FileOps for PipeMaxSizeFileOps {
    fn read_at(&self, offset: usize, writer: &mut VmWriter) -> Result<usize> {
        // Linux defaults to 1 MiB for `/proc/sys/fs/pipe-max-size`.
        const PIPE_MAX_SIZE: usize = 1 << 20;

        let mut printer = VmPrinter::new_skip(writer, offset);
        writeln!(printer, "{}", PIPE_MAX_SIZE)?;

        Ok(printer.bytes_written())
    }
}
