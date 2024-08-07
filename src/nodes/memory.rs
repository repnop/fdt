use crate::{
    cell_collector::{BuildCellCollector, CellCollector, CollectCellsError},
    parsing::{aligned::AlignedParser, NoPanic, Panic, ParserWithMode},
    properties::{
        cells::{CellSizes, SizeCells},
        reg::Reg,
        Compatible,
    },
    FdtError,
};

use super::{AsNode, FallibleNode, NodeChildrenIter, NodeName};

/// [Devicetree 3.4. `/memory`
/// node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#memory-node)
///
/// A memory device node is required for all devicetrees and describes the
/// physical memory layout for the system. If a system has multiple ranges of
/// memory, multiple memory nodes can be created, or the ranges can be specified
/// in the `reg` property of a single memory node.
///
/// The unit-name component of the node name (see [Section
/// 2.2.1](https://devicetree-specification.readthedocs.io/en/latest/chapter2-devicetree-basics.html#sect-node-names))
/// shall be memory.
///
/// The client program may access memory not covered by any memory reservations
/// (see [Section
/// 5.3](https://devicetree-specification.readthedocs.io/en/latest/chapter5-flattened-format.html#sect-fdt-memory-reservation-block))
/// using any storage attributes it chooses. However, before changing the
/// storage attributes used to access a real page, the client program is
/// responsible for performing actions required by the architecture and
/// implementation, possibly including flushing the real page from the caches.
/// The boot program is responsible for ensuring that, without taking any action
/// associated with a change in storage attributes, the client program can
/// safely access all memory (including memory covered by memory reservations)
/// as `WIMG = 0b001x`. That is:
///
/// * not Write Through Required
/// * not Caching Inhibited
/// * Memory Coherence
/// * Required either not Guarded or Guarded
///
/// If the VLE storage attribute is supported, with `VLE=0`.
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
    #[track_caller]
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
    #[track_caller]
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
    #[track_caller]
    pub fn hotpluggable(&self) -> P::Output<bool> {
        P::to_output(crate::tryblock!({ Ok(self.node.properties()?.find("hotpluggable")?.is_some()) }))
    }
}

impl<'a, P: ParserWithMode<'a>> AsNode<'a, P> for Memory<'a, P> {
    fn as_node(&self) -> super::Node<'a, P> {
        self.node.alt()
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
    #[inline]
    #[track_caller]
    pub fn cell_sizes(&self) -> P::Output<CellSizes> {
        P::to_output(
            self.node
                .property::<CellSizes>()
                .and_then(|c| c.ok_or(FdtError::MissingRequiredNode("#address-cells/#size-cells"))),
        )
    }

    #[inline]
    #[track_caller]
    pub fn children(&self) -> P::Output<ReservedMemoryChildrenIter<'a, P>> {
        P::to_output(crate::tryblock!({ Ok(ReservedMemoryChildrenIter { children: self.node.children()?.iter() }) }))
    }
}

impl<'a, P: ParserWithMode<'a>> AsNode<'a, P> for ReservedMemory<'a, P> {
    fn as_node(&self) -> super::Node<'a, P> {
        self.node.alt()
    }
}

pub struct ReservedMemoryChildrenIter<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    children: NodeChildrenIter<'a, (P::Parser, NoPanic)>,
}

impl<'a, P: ParserWithMode<'a>> Iterator for ReservedMemoryChildrenIter<'a, P> {
    type Item = P::Output<ReservedMemoryChild<'a, P>>;

    #[track_caller]
    fn next(&mut self) -> Option<Self::Item> {
        match self.children.next()? {
            Ok(node) => Some(P::to_output(Ok(ReservedMemoryChild { node }))),
            Err(e) => Some(P::to_output(Err(e))),
        }
    }
}

pub struct ReservedMemoryChild<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    node: FallibleNode<'a, P>,
}

impl<'a, P: ParserWithMode<'a>> ReservedMemoryChild<'a, P> {
    pub fn name(&self) -> P::Output<NodeName<'a>> {
        P::to_output(self.node.name())
    }

    /// [Devicetree 3.5.2. `/reserved-memory` child
    /// nodes](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#table-5)
    ///
    /// **Optional**
    ///
    /// Consists of an arbitrary number of address and size pairs that specify
    /// the physical address and size of the memory ranges.
    pub fn reg(&self) -> P::Output<Option<Reg<'a>>> {
        P::to_output(self.node.reg())
    }

    /// [Devicetree 3.5.2. `/reserved-memory` child
    /// nodes](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#table-5)
    ///
    /// **Optional**
    ///
    /// Size in bytes of memory to reserve for dynamically allocated regions.
    /// Size of this property is based on parent node’s `#size-cells` property.
    pub fn size<C: CellCollector>(&self) -> P::Output<Option<Result<C::Output, CollectCellsError>>> {
        P::to_output(crate::tryblock!({
            let Some(size) = self.node.properties()?.find("size")? else {
                return Ok(None);
            };

            // Unwrap: nodes will always have parents because they are created
            // from the `NodeChildrenIter` struct
            let size_cells = self.node.parent().unwrap().property::<SizeCells>()?.unwrap_or(SizeCells(1));

            if size.value().len() % size_cells.0 != 0 {
                return Err(FdtError::InvalidPropertyValue);
            }

            let mut builder = <C as CellCollector>::Builder::default();

            for component in size.value().chunks_exact(4) {
                if builder.push(u32::from_be_bytes(component.try_into().unwrap())).is_err() {
                    return Ok(Some(Err(CollectCellsError)));
                }
            }

            Ok(Some(Ok(C::map(builder.finish()))))
        }))
    }

    /// [Devicetree 3.5.2. `/reserved-memory` child
    /// nodes](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#table-5)
    ///
    /// **Optional**
    ///
    /// Address boundary for alignment of allocation. Size of this property is
    /// based on parent node’s `#size-cells` property.
    pub fn alignment<C: CellCollector>(&self) -> P::Output<Option<Result<C::Output, CollectCellsError>>> {
        P::to_output(crate::tryblock!({
            let Some(alignment) = self.node.properties()?.find("alignment")? else {
                return Ok(None);
            };

            // Unwrap: nodes will always have parents because they are created
            // from the `NodeChildrenIter` struct
            let size_cells = self.node.parent().unwrap().property::<SizeCells>()?.unwrap_or(SizeCells(1));

            if alignment.value().len() % size_cells.0 != 0 {
                return Err(FdtError::InvalidPropertyValue);
            }

            let mut builder = <C as CellCollector>::Builder::default();

            for component in alignment.value().chunks_exact(4) {
                if builder.push(u32::from_be_bytes(component.try_into().unwrap())).is_err() {
                    return Ok(Some(Err(CollectCellsError)));
                }
            }

            Ok(Some(Ok(C::map(builder.finish()))))
        }))
    }

    /// [Devicetree 3.5.2. `/reserved-memory` child
    /// nodes](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#table-5)
    ///
    /// **Optional**
    ///
    /// May contain the following strings:
    ///
    /// * `shared-dma-pool`: This indicates a region of memory meant to be used
    ///   as a shared pool of DMA buffers for a set of devices. It can be used by
    ///   an operating system to instantiate the necessary pool management
    ///   subsystem if necessary.
    ///
    /// * vendor specific string in the form `<vendor>,[<device>-]<usage>`
    pub fn compatible(&self) -> P::Output<Option<Compatible<'a>>> {
        P::to_output(self.node.property::<Compatible<'a>>())
    }

    /// [Devicetree 3.5.2. `/reserved-memory` child
    /// nodes](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#table-5)
    ///
    /// **Optional**
    ///
    /// If present, indicates the operating system must not create a virtual
    /// mapping of the region as part of its standard mapping of system memory,
    /// nor permit speculative access to it under any circumstances other than
    /// under the control of the device driver using the region.
    pub fn no_map(&self) -> P::Output<bool> {
        P::to_output(self.node.properties().and_then(|p| p.find("no-map").map(|p| p.is_some())))
    }

    /// [Devicetree 3.5.2. `/reserved-memory` child
    /// nodes](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#table-5)
    ///
    /// **Optional**
    ///
    /// The operating system can use the memory in this region with the
    /// limitation that the device driver(s) owning the region need to be able
    /// to reclaim it back. Typically that means that the operating system can
    /// use that region to store volatile or cached data that can be otherwise
    /// regenerated or migrated elsewhere.
    pub fn reusable(&self) -> P::Output<bool> {
        P::to_output(self.node.properties().and_then(|p| p.find("no-map").map(|p| p.is_some())))
    }

    /// [Devicetree 3.5.2. `/reserved-memory` child
    /// nodes](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#table-5)
    ///
    /// If a `linux,cma-default` property is present, then Linux will use the
    /// region for the default pool of the contiguous memory allocator.
    #[cfg(feature = "linux-dt-bindings")]
    pub fn cma_default(&self) -> P::Output<bool> {
        P::to_output(self.node.properties().and_then(|p| p.find("no-map").map(|p| p.is_some())))
    }

    /// [Devicetree 3.5.2. `/reserved-memory` child
    /// nodes](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#table-5)
    ///
    /// If a `linux,dma-default` property is present, then Linux will use the
    /// region for the default pool of the consistent DMA allocator.
    #[cfg(feature = "linux-dt-bindings")]
    pub fn dma_default(&self) -> P::Output<bool> {
        P::to_output(self.node.properties().and_then(|p| p.find("no-map").map(|p| p.is_some())))
    }
}

impl<'a, P: ParserWithMode<'a>> AsNode<'a, P> for ReservedMemoryChild<'a, P> {
    fn as_node(&self) -> super::Node<'a, P> {
        self.node.alt()
    }
}

/// A memory region
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoryRegion {
    pub starting_address: u64,
    pub size: Option<usize>,
}
