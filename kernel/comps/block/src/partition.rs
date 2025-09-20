// SPDX-License-Identifier: MPL-2.0

use alloc::{format, sync::Arc};

use aster_systree::{SysObj, SysStr};
use ostd::{mm::VmIo, Pod};

use crate::{
    prelude::*,
    sysnode::{BlockCommonInfo, BlockExtraInfo, BlockSysNode, PartitionInfo},
    BlockDevice, SECTOR_SIZE,
};

/// Represents a partition entry.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Partition {
    Mbr(MbrEntry),
    Gpt(GptEntry),
}

impl Partition {
    fn start_sector(&self) -> u64 {
        match self {
            Partition::Mbr(entry) => entry.start_sector as u64,
            Partition::Gpt(entry) => entry.start_lba,
        }
    }

    fn total_sectors(&self) -> u64 {
        match self {
            Partition::Mbr(entry) => entry.total_sectors as u64,
            Partition::Gpt(entry) => entry.end_lba - entry.start_lba + 1,
        }
    }
}

/// A MBR (Master Boot Record) partition table header.
///
/// See <https://wiki.osdev.org/MBR_(x86)#MBR_Format>.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
struct MbrHeader {
    bootstrap_code: [u8; 440],
    id: u32,
    reserved: u16,
    entries: [MbrEntry; 4],
    signature: u16,
}

impl MbrHeader {
    fn check_signature(&self) -> bool {
        self.signature == 0xAA55
    }
}

/// A MBR (Master Boot Record) partition entry.
///
/// See <https://wiki.osdev.org/Partition_Table>.
#[repr(C, packed)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Pod)]
struct MbrEntry {
    flag: u8,
    start_chs: ChsAddr,
    type_: u8,
    end_chs: ChsAddr,
    start_sector: u32,
    total_sectors: u32,
}

impl MbrEntry {
    fn is_extended(&self) -> bool {
        self.type_ == 0x05 || self.type_ == 0x0F
    }

    fn is_valid(&self) -> bool {
        // A System ID byte value of 0 is the definitive indicator for an unused entry.
        // Any other illegal value (CHS Sector = 0 or Total Sectors = 0) may also indicate an unused entry.
        self.type_ != 0x00
            && self.start_chs.0[1] != 0
            && self.end_chs.0[1] != 0
            && self.total_sectors != 0
    }
}

/// A CHS (Cylinder-Head-Sector) address.
///
/// In CHS addressing, sector numbers always start at 1; there is no sector 0.
///
/// The CHS address is stored as a 3-byte field:
/// - Byte 0: Head number (8 bits)
/// - Byte 1: Bits 0–5 are the sector number (6 bits, valid values 1–63);
///   bits 6–7 are the upper two bits of the cylinder number
/// - Byte 2: Lower 8 bits of the cylinder number (bits 0–7)
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Pod)]
struct ChsAddr([u8; 3]);

/// A GPT (GUID Partition Table) header.
///
/// See <https://wiki.osdev.org/GPT#LBA_1:_Partition_Table_Header>.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod)]
struct GptHeader {
    signature: u64,
    revision: u32,
    size: u32,
    crc32: u32,
    reserved: u32,
    current_lba: u64,
    backup_lba: u64,
    first_usable_lba: u64,
    last_usable_lba: u64,
    guid: [u8; 16],
    partition_entry_lba: u64,
    nr_partition_entries: u32,
    size_of_partition_entry: u32,
    crc32_of_partition_entries: u32,
    _padding: [u8; 420],
}

impl GptHeader {
    fn check_signature(&self) -> bool {
        &self.signature.to_le_bytes() == b"EFI PART"
    }
}

/// A GPT (GUID Partition Table) partition entry.
///
/// See <https://wiki.osdev.org/GPT#LBA_2:_Partition_Entries>.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq, Pod)]
struct GptEntry {
    // Unique ID that defines the purpose and type of this Partition.
    // A value of zero defines that this partition entry is not being used.
    type_guid: [u8; 16],
    // GUID that is unique for every partition entry.
    guid: [u8; 16],
    start_lba: u64,
    end_lba: u64,
    attributes: u64,
    // Null-terminated string containing a human-readable name of the partition.
    name: [u8; 72],
}

impl GptEntry {
    fn is_valid(&self) -> bool {
        self.type_guid != [0; 16]
    }
}

/// If a disk has more than 16 partitions, the extended major:minor numbers will be assigned.
const LEGACY_PARTITION_LIMIT: u32 = 16;

/// The major device number used for extended partitions when the number
/// of disk partitions exceeds the standard limit.
const EXTENDED_MAJOR: u32 = 259;

/// The next available minor device number for extended partitions.
static EXTENDED_MINOR: AtomicU32 = AtomicU32::new(0);

pub(super) fn parse(device: &Arc<dyn BlockDevice>) {
    let mbr = device.read_val::<MbrHeader>(0).unwrap();

    // 0xEE indicates a GPT Protective MBR, a fake partition covering the entire disk.
    let partitions = if mbr.check_signature() && mbr.entries[0].type_ != 0xEE {
        parse_mbr(device, &mbr)
    } else {
        parse_gpt(device)
    };

    add_partition_sysnodes(device, partitions);
}

fn parse_mbr(device: &Arc<dyn BlockDevice>, mbr: &MbrHeader) -> Vec<Option<Partition>> {
    let mut partitions = Vec::new();
    let mut extended_partition = None;
    for entry in mbr.entries {
        if entry.is_extended() {
            extended_partition = Some(entry.start_sector);
        }

        if entry.is_valid() {
            partitions.push(Some(Partition::Mbr(entry)));
        } else {
            partitions.push(None);
        }
    }

    if let Some(start_sector) = extended_partition {
        parse_ebr(device, &mut partitions, start_sector, 0);
    }

    partitions
}

fn parse_ebr(
    device: &Arc<dyn BlockDevice>,
    partitions: &mut Vec<Option<Partition>>,
    start_sector: u32,
    offset: u32,
) {
    let ebr_sector = start_sector + offset;
    let mut ebr = device
        .read_val::<MbrHeader>(ebr_sector as usize * SECTOR_SIZE)
        .unwrap();
    if ebr.entries[0].is_valid() {
        ebr.entries[0].start_sector += ebr_sector;
        partitions.push(Some(Partition::Mbr(ebr.entries[0])));
    }

    if ebr.entries[1].is_extended() {
        parse_ebr(
            device,
            partitions,
            start_sector,
            ebr.entries[1].start_sector,
        );
    }
}

fn parse_gpt(device: &Arc<dyn BlockDevice>) -> Vec<Option<Partition>> {
    let mut partitions = Vec::new();

    // The primary GPT Header must be located in LBA 1.
    let gpt = device.read_val::<GptHeader>(SECTOR_SIZE).unwrap();

    if !gpt.check_signature() {
        return partitions;
    }

    // TODO: Check the CRC32 of the header and the partition entries, check the backup GPT header.

    let entry_size = gpt.size_of_partition_entry as usize;
    let entries_per_sector = SECTOR_SIZE / entry_size;
    let total_sectors = gpt.nr_partition_entries as usize / entries_per_sector;
    for i in 0..total_sectors {
        let mut buf = [0u8; SECTOR_SIZE];
        let offset = (gpt.partition_entry_lba as usize + i) * SECTOR_SIZE;
        device.read_bytes(offset, buf.as_mut_slice()).unwrap();

        for j in 0..entries_per_sector {
            let entry_offset = j * gpt.size_of_partition_entry as usize;
            let entry = GptEntry::from_bytes(&buf[entry_offset..entry_offset + entry_size]);
            if entry.is_valid() {
                partitions.push(Some(Partition::Gpt(entry)));
            } else {
                partitions.push(None);
            }
        }
    }

    partitions
}

fn add_partition_sysnodes(device: &Arc<dyn BlockDevice>, partitions: Vec<Option<Partition>>) {
    let sysnode = device.sysnode();
    let sysnode = Arc::downcast::<BlockSysNode>(sysnode).unwrap();
    for (index, partition) in partitions.into_iter().enumerate() {
        let Some(partition) = partition else {
            continue;
        };

        let id = index as u32 + 1;
        let name = format!("{}{}", sysnode.name(), id);

        let (major, minor) = if id < LEGACY_PARTITION_LIMIT {
            (sysnode.common_info.major, sysnode.common_info.minor + id)
        } else {
            (
                EXTENDED_MAJOR,
                EXTENDED_MINOR.fetch_add(1, Ordering::Relaxed),
            )
        };
        let common_info = BlockCommonInfo {
            major,
            minor,
            size: partition.total_sectors(),
        };

        let partition_info = PartitionInfo {
            id,
            start: partition.start_sector(),
        };
        let extra_info = BlockExtraInfo::Partition(partition_info);

        let partition_node = BlockSysNode::new(SysStr::from(name), common_info, extra_info);
        sysnode.add_child(partition_node).unwrap();
    }
}
