use crate::{
    parsing::{aligned::AlignedParser, Panic, ParserWithMode},
    properties::{cells::CellSizes, reg::Reg},
    FdtError,
};

use super::FallibleNode;

/// Represents the `/memory` node with specific helper methods
#[derive(Debug, Clone, Copy)]
pub struct Memory<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    pub(crate) node: FallibleNode<'a, P>,
}

impl<'a, P: ParserWithMode<'a>> Memory<'a, P> {
    /// [Devicetree 3.4. `/memory`
    /// node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#memory-node)
    ///
    /// **Required**
    ///
    /// Consists of an arbitrary number of address and size pairs that specify
    /// the physical address and size of the memory ranges.
    pub fn reg(&self) -> P::Output<Reg<'a>> {
        P::to_output(self.node.reg().and_then(|m| m.ok_or(FdtError::MissingRequiredNode("reg"))))
    }

    /// [Devicetree 3.4. `/memory`
    /// node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#memory-node)
    ///
    /// **Optional**
    ///
    /// Specifies the address and size of the Initial Mapped Area
    ///
    /// A `prop-encoded-array` consisting of a triplet of (effective address,
    /// physical address, size). The effective and physical address shall each
    /// be 64-bit (`<u64>` value), and the size shall be 32-bits (`<u32>`
    /// value).
    pub fn initial_mapped_area(&self) -> P::Output<Option<MappedArea>> {
        P::to_output(crate::tryblock!({
            match self.node.properties()?.find("initial-mapped-area")? {
                Some(prop) => {
                    let value = prop.value();
                    if value.len() != (/* effective address */8 + /* physical address */ 8 + /* size */ 4) {
                        return Err(FdtError::InvalidPropertyValue);
                    }

                    Ok(Some(MappedArea {
                        effective_address: u64::from_be_bytes(value[0..8].try_into().unwrap()),
                        physical_address: u64::from_be_bytes(value[8..16].try_into().unwrap()),
                        size: u32::from_be_bytes(value[16..20].try_into().unwrap()),
                    }))
                }
                None => Ok(None),
            }
        }))
    }

    /// [Devicetree 3.4. `/memory`
    /// node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#memory-node)
    ///
    /// Specifies an explicit hint to the operating system that this memory may
    /// potentially be removed later.
    pub fn hotpluggable(&self) -> P::Output<bool> {
        P::to_output(crate::tryblock!({ Ok(self.node.properties()?.find("hotpluggable")?.is_some()) }))
    }
}

/// Describes the initial mapped area of the `/memory` node. See
/// [`Memory::initial_mapped_area`].
#[allow(missing_docs)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MappedArea {
    pub effective_address: u64,
    pub physical_address: u64,
    pub size: u32,
}

/// [Devicetree 3.5. `/reserved-memory`
/// node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#reserved-memory-node)
///
/// Reserved memory is specified as a node under the `/reserved-memory` node. The
/// operating system shall exclude reserved memory from normal usage. One can
/// create child nodes describing particular reserved (excluded from normal use)
/// memory regions. Such memory regions are usually designed for the special
/// usage by various device drivers.
pub struct ReservedMemory<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    pub(crate) node: FallibleNode<'a, P>,
}

impl<'a, P: ParserWithMode<'a>> ReservedMemory<'a, P> {
    pub fn cell_sizes(&self) -> P::Output<CellSizes> {
        P::to_output(
            self.node
                .property::<CellSizes>()
                .and_then(|c| c.ok_or(FdtError::MissingRequiredNode("#address-cells/#size-cells"))),
        )
    }

    pub fn children(&self) -> ReservedMemoryChildren<'a, P> {
        ReservedMemoryChildren { node: self.node }
    }
}

pub struct ReservedMemoryChildren<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    node: FallibleNode<'a, P>,
}

impl<'a, P: ParserWithMode<'a>> ReservedMemoryChildren<'a, P> {
    pub fn all(&self) -> P::Output<ReservedMemoryChildrenIter<'a, P>> {}
}

pub struct ReservedMemoryChild<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    node: FallibleNode<'a, P>,
}

/// A memory region
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoryRegion {
    pub starting_address: u64,
    pub size: Option<usize>,
}

/// Range mapping child bus addresses to parent bus addresses
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoryRange {
    /// Starting address on child bus
    pub child_bus_address: usize,
    /// The high bits of the child bus' starting address, if present
    pub child_bus_address_hi: u32,
    /// Starting address on parent bus
    pub parent_bus_address: usize,
    /// Size of range
    pub size: usize,
}
