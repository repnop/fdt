use super::{
    cells::{AddressCells, CellSizes},
    Property,
};
use crate::{
    cell_collector::{BuildCellCollector, CellCollector, CollectCellsError},
    nodes::{root::Root, FallibleNode},
    parsing::{NoPanic, ParserWithMode},
    FdtError,
};

#[cfg(doc)]
use crate::nodes::Node;

/// See [`Node::ranges`].
#[derive(Debug, Clone, Copy)]
pub struct Ranges<'a> {
    parent_address_cells: AddressCells,
    cell_sizes: CellSizes,
    ranges: &'a [u8],
}

impl<'a> Ranges<'a> {
    pub fn iter<CAddr, PAddr, Len>(self) -> RangesIter<'a, CAddr, PAddr, Len>
    where
        CAddr: CellCollector,
        PAddr: CellCollector,
        Len: CellCollector,
    {
        RangesIter {
            parent_address_cells: self.parent_address_cells,
            cell_sizes: self.cell_sizes,
            ranges: self.ranges,
            _collectors: core::marker::PhantomData,
        }
    }
}

impl<'a, P: ParserWithMode<'a>> Property<'a, P> for Ranges<'a> {
    fn parse(
        node: FallibleNode<'a, P>,
        _: Root<'a, (<P as ParserWithMode<'a>>::Parser, NoPanic)>,
    ) -> Result<Option<Self>, FdtError> {
        let Some(ranges) = node.properties()?.find("ranges")? else {
            return Ok(None);
        };

        let parent_address_cells =
            node.parent().ok_or(FdtError::MissingParent)?.property::<AddressCells>()?.unwrap_or_default();
        let cell_sizes = node.property::<CellSizes>()?.unwrap_or_default();

        Ok(Some(Self { parent_address_cells, cell_sizes, ranges: ranges.value() }))
    }
}

pub struct RangesIter<'a, CAddr: CellCollector = u64, PAddr: CellCollector = u64, Len: CellCollector = u64> {
    parent_address_cells: AddressCells,
    cell_sizes: CellSizes,
    ranges: &'a [u8],
    _collectors: core::marker::PhantomData<*mut (CAddr, PAddr, Len)>,
}

impl<'a, CAddr: CellCollector, PAddr: CellCollector, Len: CellCollector> Iterator
    for RangesIter<'a, CAddr, PAddr, Len>
{
    type Item = Result<Range<CAddr::Output, PAddr::Output, Len::Output>, CollectCellsError>;
    fn next(&mut self) -> Option<Self::Item> {
        let child_address_bytes = self.cell_sizes.address_cells * 4;
        let parent_address_bytes = self.parent_address_cells.0 * 4;
        let len_bytes = self.cell_sizes.size_cells * 4;

        let child_encoded_address = self.ranges.get(..child_address_bytes)?;
        let parent_encoded_address =
            self.ranges.get(child_address_bytes..child_address_bytes + parent_address_bytes)?;
        let encoded_len = self
            .ranges
            .get(child_address_bytes + parent_address_bytes..child_address_bytes + parent_address_bytes + len_bytes)?;

        let mut child_address_collector = <CAddr as CellCollector>::Builder::default();
        for encoded_address in child_encoded_address.chunks_exact(4) {
            // TODO: replace this stuff with `array_chunks` when its stabilized
            //
            // These unwraps can't panic because `chunks_exact` guarantees that
            // we'll always get slices of 4 bytes
            if let Err(e) = child_address_collector.push(u32::from_be_bytes(encoded_address.try_into().unwrap())) {
                return Some(Err(e));
            }
        }

        let mut parent_address_collector = <PAddr as CellCollector>::Builder::default();
        for encoded_address in parent_encoded_address.chunks_exact(4) {
            // TODO: replace this stuff with `array_chunks` when its stabilized
            //
            // These unwraps can't panic because `chunks_exact` guarantees that
            // we'll always get slices of 4 bytes
            if let Err(e) = parent_address_collector.push(u32::from_be_bytes(encoded_address.try_into().unwrap())) {
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

        self.ranges = self.ranges.get(child_address_bytes + parent_address_bytes + len_bytes..)?;
        Some(Ok(Range {
            child_bus_address: CAddr::map(child_address_collector.finish()),
            parent_bus_address: PAddr::map(parent_address_collector.finish()),
            len: Len::map(len_collector.finish()),
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Range<CAddr, PAddr, Len> {
    pub child_bus_address: CAddr,
    pub parent_bus_address: PAddr,
    pub len: Len,
}
