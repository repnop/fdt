use super::{cells::CellSizes, Property};
use crate::{
    cell_collector::{BuildCellCollector, CellCollector, CollectCellsError},
    helpers::{FallibleNode, FallibleRoot},
    parsing::ParserWithMode,
    FdtError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reg<'a> {
    cell_sizes: CellSizes,
    encoded_array: &'a [u8],
}

impl<'a> Reg<'a> {
    pub fn cell_sizes(self) -> CellSizes {
        self.cell_sizes
    }

    pub fn iter_raw(self) -> RegRawIter<'a> {
        RegRawIter { cell_sizes: self.cell_sizes, encoded_array: self.encoded_array }
    }

    pub fn iter<Addr: CellCollector, Len: CellCollector>(self) -> RegIter<'a, Addr, Len> {
        RegIter {
            cell_sizes: self.cell_sizes,
            encoded_array: self.encoded_array,
            _collector: core::marker::PhantomData,
        }
    }
}

impl<'a, P: ParserWithMode<'a>> Property<'a, P> for Reg<'a> {
    fn parse(node: FallibleNode<'a, P>, _: FallibleRoot<'a, P>) -> Result<Option<Self>, FdtError> {
        let Some(prop) = node.raw_property("reg")? else {
            return Ok(None);
        };

        let cell_sizes = match node.parent() {
            Some(parent) => parent.property::<CellSizes>()?.unwrap_or_default(),
            None => CellSizes::default(),
        };

        let encoded_array = prop.value;

        if encoded_array.len() % (cell_sizes.address_cells * 4 + cell_sizes.size_cells * 4) != 0 {
            return Err(FdtError::InvalidPropertyValue);
        }

        Ok(Some(Self { cell_sizes, encoded_array }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegEntry<Addr, Len> {
    pub address: Addr,
    pub len: Len,
}

pub struct RegIter<'a, CAddr: CellCollector, Len: CellCollector> {
    cell_sizes: CellSizes,
    encoded_array: &'a [u8],
    _collector: core::marker::PhantomData<*mut (CAddr, Len)>,
}

impl<'a, CAddr: CellCollector, Len: CellCollector> Iterator for RegIter<'a, CAddr, Len> {
    type Item = Result<RegEntry<CAddr::Output, Len::Output>, CollectCellsError>;
    fn next(&mut self) -> Option<Self::Item> {
        let address_bytes = self.cell_sizes.address_cells * 4;
        let size_bytes = self.cell_sizes.size_cells * 4;

        let encoded_address = self.encoded_array.get(..address_bytes)?;
        let encoded_len = self.encoded_array.get(address_bytes..address_bytes + size_bytes)?;

        let mut address_collector = <CAddr as CellCollector>::Builder::default();
        for encoded_address in encoded_address.chunks_exact(4) {
            // TODO: replace this stuff with `array_chunks` when its stabilized
            //
            // These unwraps can't panic because `chunks_exact` guarantees that
            // we'll always get slices of 4 bytes
            if let Err(e) = address_collector.push(u32::from_be_bytes(encoded_address.try_into().unwrap())) {
                return Some(Err(e));
            }
        }

        let mut len_collector = <Len as CellCollector>::Builder::default();
        for encoded_len in encoded_len.chunks_exact(4) {
            // TODO: replace this stuff with `array_chunks` when its stabilized
            //
            // These unwraps can't panic because `chunks_exact` guarantees that
            // we'll always get slices of 4 bytes
            if let Err(e) = len_collector.push(u32::from_be_bytes(encoded_len.try_into().unwrap())) {
                return Some(Err(e));
            }
        }

        self.encoded_array = self.encoded_array.get((address_bytes + size_bytes)..)?;
        Some(Ok(RegEntry { address: CAddr::map(address_collector.finish()), len: Len::map(len_collector.finish()) }))
    }
}

pub struct RegRawIter<'a> {
    cell_sizes: CellSizes,
    encoded_array: &'a [u8],
}

impl<'a> Iterator for RegRawIter<'a> {
    type Item = RawRegEntry<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let address_bytes = self.cell_sizes.address_cells * 4;
        let size_bytes = self.cell_sizes.size_cells * 4;

        let (addr_len, rest) = self.encoded_array.split_at_checked(address_bytes + size_bytes)?;
        let (address, len) = addr_len.split_at(address_bytes);

        self.encoded_array = rest;
        Some(RawRegEntry { address, len })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawRegEntry<'a> {
    pub address: &'a [u8],
    pub len: &'a [u8],
}

/// [Devicetree 2.3.7.
/// `virtual-reg`](https://devicetree-specification.readthedocs.io/en/latest/chapter2-devicetree-basics.html#virtual-reg)
///
/// The `virtual-reg` property specifies an effective address that maps to the
/// first physical address specified in the `reg` property of the device node.
/// This property enables boot programs to provide client programs with
/// virtual-to-physical mappings that have been set up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VirtualReg(u32);

impl VirtualReg {
    pub fn into_u32(self) -> u32 {
        self.0
    }
}

impl<'a, P: ParserWithMode<'a>> Property<'a, P> for VirtualReg {
    fn parse(node: FallibleNode<'a, P>, _: FallibleRoot<'a, P>) -> Result<Option<Self>, FdtError> {
        match node.properties()?.find("virtual-reg")? {
            Some(vreg) => Ok(Some(Self(vreg.as_value()?))),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reg_raw_iter() {
        let mut iter = RegRawIter {
            cell_sizes: CellSizes { address_cells: 2, size_cells: 1 },
            encoded_array: &[0x55, 0x44, 0x33, 0x22, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD],
        };

        assert_eq!(
            iter.next().unwrap(),
            RawRegEntry { address: &[0x55, 0x44, 0x33, 0x22, 0x66, 0x77, 0x88, 0x99], len: &[0xAA, 0xBB, 0xCC, 0xDD] }
        );
    }

    #[test]
    fn reg_u64_iter() {
        let mut iter = RegIter::<u64, usize> {
            cell_sizes: CellSizes { address_cells: 2, size_cells: 1 },
            encoded_array: &[0x55, 0x44, 0x33, 0x22, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD],
            _collector: core::marker::PhantomData,
        };

        assert_eq!(iter.next().unwrap().unwrap(), RegEntry { address: 0x5544332266778899, len: 0xAABBCCDD });
    }
}
