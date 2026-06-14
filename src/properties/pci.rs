use crate::cell_collector::{BuildCellCollector, CellCollector, CollectCellsError};

/// [PCI Bus Binding to Open Firmware 2.2.1.1 Numerical Representation](https://www.openfirmware.info/data/docs/bus.pci.pdf)
///
/// Numerical representation of a PCI address used within the `interrupt-map` property
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PciAddress {
    #[allow(missing_docs)]
    pub hi: PciAddressHighBits,
    #[allow(missing_docs)]
    pub mid: u32,
    #[allow(missing_docs)]
    pub lo: u32,
}

impl CellCollector for PciAddress {
    type Builder = PciAddressCollector;
    type Output = Self;

    fn map(builder_out: <Self::Builder as BuildCellCollector>::Output) -> Self::Output {
        builder_out
    }
}

impl PartialEq<&'_ PciAddress> for PciAddress {
    fn eq(&self, other: &&'_ Self) -> bool {
        self.eq(*other)
    }
}

impl PartialEq<PciAddress> for &'_ PciAddress {
    fn eq(&self, other: &PciAddress) -> bool {
        (*self).eq(other)
    }
}

/// `phys.hi cell: npt000ss bbbbbbbb dddddfff rrrrrrrr`
///
/// where:
///
/// `n` is 0 if the address is relocatable, 1 otherwise
///
/// `p` is 1 if the addressable region is "prefetchable", 0 otherwise
///
/// `t` is 1 if the address is aliased (for non-relocatable I/O), below 1 MB (for Memory),
///
/// `or` below 64 KB (for relocatable I/O).
///
/// `ss` is the space code, denoting the address space
///
/// `bbbbbbbb` is the 8-bit Bus Number
///
/// `ddddd` is the 5-bit Device Number
///
/// `fff` is the 3-bit Function Number
///
/// `rrrrrrrr` is the 8-bit Register Number
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PciAddressHighBits(u32);

#[allow(missing_docs)]
impl PciAddressHighBits {
    #[inline(always)]
    pub fn new(raw: u32) -> Self {
        Self(raw)
    }

    #[inline(always)]
    pub fn register(self) -> u8 {
        self.0 as u8
    }

    #[inline(always)]
    pub fn function(self) -> u8 {
        ((self.0 >> 8) & 0b111) as u8
    }

    #[inline(always)]
    pub fn device(self) -> u8 {
        ((self.0 >> 12) & 0b11111) as u8
    }

    #[inline(always)]
    pub fn bus(self) -> u8 {
        (self.0 >> 16) as u8
    }

    #[inline(always)]
    pub fn address_space(self) -> PciAddressSpace {
        const CONFIGURATION: u8 = const { PciAddressSpace::Configuration as u8 };
        const IO: u8 = const { PciAddressSpace::Io as u8 };
        const MEMORY32: u8 = const { PciAddressSpace::Memory32 as u8 };
        const MEMORY64: u8 = const { PciAddressSpace::Memory64 as u8 };

        match ((self.0 >> 24) & 0b11) as u8 {
            CONFIGURATION => PciAddressSpace::Configuration,
            IO => PciAddressSpace::Io,
            MEMORY32 => PciAddressSpace::Memory32,
            MEMORY64 => PciAddressSpace::Memory64,
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    pub fn prefetchable(self) -> bool {
        (self.0 >> 30) & 0b1 == 0b1
    }

    #[inline(always)]
    pub fn relocatable(self) -> bool {
        (self.0 >> 31) & 0b1 == 0b0
    }
}

impl core::ops::BitAnd for PciAddress {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        Self { hi: PciAddressHighBits(self.hi.0 & rhs.hi.0), mid: self.mid & rhs.mid, lo: self.lo & rhs.lo }
    }
}

/// Type of PCI address space.
#[allow(missing_docs)]
#[repr(u8)]
pub enum PciAddressSpace {
    Configuration = 0b00,
    Io = 0b01,
    Memory32 = 0b10,
    Memory64 = 0b11,
}

#[allow(missing_docs)]
#[derive(Default)]
pub struct PciAddressCollector {
    address: PciAddress,
    num_pushes: u32,
}

impl BuildCellCollector for PciAddressCollector {
    type Output = PciAddress;

    fn push(&mut self, component: u32) -> Result<(), CollectCellsError> {
        match self.num_pushes {
            0 => self.address.hi = PciAddressHighBits(component),
            1 => self.address.mid = component,
            2 => self.address.lo = component,
            _ => return Err(CollectCellsError),
        }

        self.num_pushes += 1;

        Ok(())
    }

    fn finish(self) -> Self::Output {
        self.address
    }
}
