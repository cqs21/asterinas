// SPDX-License-Identifier: MPL-2.0

pub(super) mod acpi;
pub(super) mod apic;
pub(super) mod pic;
pub(super) mod tsc;

pub use acpi::ACPI_TABLES;
pub use apic::ioapic::IO_APIC;
