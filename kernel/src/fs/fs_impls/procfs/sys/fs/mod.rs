// SPDX-License-Identifier: MPL-2.0

use aster_util::slot_vec::SlotVec;
use ostd::sync::RwMutexUpgradeableGuard;

use self::pipe_max_size::PipeMaxSizeFileOps;
use crate::{
    fs::{
        file::mkmod,
        procfs::template::{
            DirOps, ProcDir, ProcDirBuilder, lookup_child_from_table, populate_children_from_table,
        },
        vfs::inode::Inode,
    },
    prelude::*,
};

mod pipe_max_size;

/// Represents the inode at `/proc/sys/fs`.
pub struct FsDirOps;

impl FsDirOps {
    pub fn new_inode(parent: Weak<dyn Inode>) -> Arc<dyn Inode> {
        ProcDirBuilder::new(Self, mkmod!(a+rx))
            .parent(parent)
            .build()
            .unwrap()
    }

    #[expect(clippy::type_complexity)]
    const STATIC_ENTRIES: &'static [(&'static str, fn(Weak<dyn Inode>) -> Arc<dyn Inode>)] =
        &[("pipe-max-size", PipeMaxSizeFileOps::new_inode)];
}

impl DirOps for FsDirOps {
    fn lookup_child(&self, dir: &ProcDir<Self>, name: &str) -> Result<Arc<dyn Inode>> {
        let mut cached_children = dir.cached_children().write();

        if let Some(child) =
            lookup_child_from_table(name, &mut cached_children, Self::STATIC_ENTRIES, |f| {
                (f)(dir.this_weak().clone())
            })
        {
            return Ok(child);
        }

        return_errno_with_message!(Errno::ENOENT, "the file does not exist");
    }

    fn populate_children<'a>(
        &self,
        dir: &'a ProcDir<Self>,
    ) -> RwMutexUpgradeableGuard<'a, SlotVec<(String, Arc<dyn Inode>)>> {
        let mut cached_children = dir.cached_children().write();

        populate_children_from_table(&mut cached_children, Self::STATIC_ENTRIES, |f| {
            (f)(dir.this_weak().clone())
        });

        cached_children.downgrade()
    }
}
