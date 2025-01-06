use super::{AsNode, FallibleNode, NodeChildrenIter};
use crate::{
    cell_collector::{BuildCellCollector, CellCollector, CollectCellsError},
    parsing::{aligned::AlignedParser, NoPanic, Panic, ParserWithMode},
    properties::{
        cells::{AddressCells, CellSizes},
        values::StringList,
        PHandle,
    },
    FdtError,
};

/// [Devicetree 3.7.
/// `/cpus`](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#cpus-node-properties)
///
/// A `/cpus` node is required for all devicetrees. It does not represent a real
/// device in the system, but acts as a container for child cpu nodes which
/// represent the systems CPUs.
pub struct Cpus<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    pub(crate) node: FallibleNode<'a, P>,
}

impl<'a, P: ParserWithMode<'a>> Cpus<'a, P> {
    /// Retrieve the `#address-cells` and `#size-cells` values from this node
    #[track_caller]
    pub fn cell_sizes(&self) -> P::Output<CellSizes> {
        P::to_output(
            self.node.property().and_then(|p| p.ok_or(FdtError::MissingRequiredProperty("#address-cells/#size-cells"))),
        )
    }

    /// Attempt to find a common `timebase-frequency` property inside of this
    /// node, which will only exist if there is a common value between the child
    /// `cpu` nodes. See [`Cpu::timebase_frequency`] for documentation about the
    /// `timebase-frequency` property.
    #[track_caller]
    pub fn common_timebase_frequency(&self) -> P::Output<Option<u64>> {
        P::to_output(crate::tryblock!({
            match self.node.properties()?.find("timebase-frequency")? {
                Some(prop) => match prop.value().len() {
                    4 => Ok(Some(u64::from(prop.as_value::<u32>()?))),
                    8 => Ok(Some(prop.as_value::<u64>()?)),
                    _ => Err(FdtError::InvalidPropertyValue),
                },
                None => Ok(None),
            }
        }))
    }

    /// Attempt to find a common `clock-frequency` property inside of this
    /// node, which will only exist if there is a common value between the child
    /// `cpu` nodes. See [`Cpu::clock_frequency`] for documentation about the
    /// `clock-frequency` property.
    #[track_caller]
    pub fn common_clock_frequency(&self) -> P::Output<Option<u64>> {
        P::to_output(crate::tryblock!({
            match self.node.properties()?.find("clock-frequency")? {
                Some(prop) => match prop.value().len() {
                    4 => Ok(Some(u64::from(prop.as_value::<u32>()?))),
                    8 => Ok(Some(prop.as_value::<u64>()?)),
                    _ => Err(FdtError::InvalidPropertyValue),
                },
                None => Ok(None),
            }
        }))
    }

    /// Returns the (optional) `cpu-map` child node, which describes the system
    /// socket and CPU topology. See [`CpuTopology`] for more details.
    pub fn topology(&self) -> P::Output<Option<CpuTopology<'a, P>>> {
        P::to_output(crate::tryblock!({
            match self.node.children()?.find("cpu-map")? {
                Some(node) => Ok(Some(CpuTopology { node })),
                None => Ok(None),
            }
        }))
    }

    pub fn iter(&self) -> P::Output<CpusIter<'a, P>> {
        P::to_output(crate::tryblock!({
            Ok(CpusIter { children: self.node.children()?.iter().filter(filter_cpus::<P>) })
        }))
    }
}

impl<'a, P: ParserWithMode<'a>> AsNode<'a, P> for Cpus<'a, P> {
    fn as_node(&self) -> super::Node<'a, P> {
        self.node.alt()
    }
}

fn filter_cpus<'a, P: ParserWithMode<'a>>(node: &Result<FallibleNode<'a, P>, FdtError>) -> bool {
    match node {
        Ok(node) => match node.name().map(|n| n.name) {
            Ok("cpu") => true,
            _ => false,
        },
        _ => true,
    }
}

pub struct CpusIter<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    children: core::iter::Filter<
        NodeChildrenIter<'a, (P::Parser, NoPanic)>,
        fn(&Result<FallibleNode<'a, P>, FdtError>) -> bool,
    >,
}

impl<'a, P: ParserWithMode<'a>> Iterator for CpusIter<'a, P> {
    type Item = P::Output<Cpu<'a, P>>;

    #[track_caller]
    fn next(&mut self) -> Option<Self::Item> {
        match self.children.next()? {
            Ok(node) => Some(P::to_output(Ok(Cpu { node }))),
            Err(e) => Some(P::to_output(Err(e))),
        }
    }
}

/// [Devicetree 3.8.
/// `/cpus/cpu*`](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#cpus-cpu-node-properties)
///
/// A `cpu` node represents a hardware execution block that is sufficiently
/// independent that it is capable of running an operating system without
/// interfering with other CPUs possibly running other operating systems.
///
/// Hardware threads that share an MMU would generally be represented under one
/// `cpu` node. If other more complex CPU topographies are designed, the binding
/// for the CPU must describe the topography (e.g. threads that don’t share an
/// MMU).
///
/// CPUs and threads are numbered through a unified number-space that should
/// match as closely as possible the interrupt controller’s numbering of
/// CPUs/threads.
///
/// Properties that have identical values across `cpu` nodes may be placed in the
/// /cpus node instead. A client program must first examine a specific `cpu` node,
/// but if an expected property is not found then it should look at the parent
/// /cpus node. This results in a less verbose representation of properties
/// which are identical across all CPUs.
#[derive(Debug, Clone, Copy)]
pub struct Cpu<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    pub(crate) node: FallibleNode<'a, P>,
}

impl<'a, P: ParserWithMode<'a>> Cpu<'a, P> {
    /// [Devicetree 3.8.1 General Properties of `/cpus/cpu*`
    /// nodes](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#general-properties-of-cpus-cpu-nodes)
    ///
    /// **Required**
    ///
    /// The value of `reg` is a `<prop-encoded-array>` that defines a unique
    /// CPU/thread id for the CPU/threads represented by the CPU node.
    ///
    /// If a CPU supports more than one thread (i.e. multiple streams of
    /// execution) the `reg` property is an array with 1 element per thread. The
    /// `#address-cells` on the `/cpus` node specifies how many cells each
    /// element of the array takes. Software can determine the number of threads
    /// by dividing the size of `reg` by the parent node’s `#address-cells`.
    ///
    /// If a CPU/thread can be the target of an external interrupt the `reg`
    /// property value must be a unique CPU/thread id that is addressable by the
    /// interrupt controller.
    ///
    /// If a CPU/thread cannot be the target of an external interrupt, then
    /// `reg` must be unique and out of bounds of the range addressed by the
    /// interrupt controller
    ///
    /// If a CPU/thread’s PIR (pending interrupt register) is modifiable, a
    /// client program should modify PIR to match the `reg` property value. If
    /// PIR cannot be modified and the PIR value is distinct from the interrupt
    /// controller number space, the CPUs binding may define a binding-specific
    /// representation of PIR values if desired.
    #[inline]
    #[track_caller]
    #[doc(alias = "ids")]
    pub fn reg<C: CellCollector>(self) -> P::Output<CpuIds<'a, C>> {
        P::to_output(crate::tryblock!({
            let Some(reg) = self.node.properties()?.find("reg")? else {
                return Err(FdtError::MissingRequiredProperty("reg"));
            };

            if reg.value().is_empty() {
                return Err(FdtError::InvalidPropertyValue);
            }

            let Some(address_cells) = self.node.parent().unwrap().property::<AddressCells>()? else {
                return Err(FdtError::MissingRequiredProperty("#address-cells"));
            };

            Ok(CpuIds { reg: reg.value(), address_cells: address_cells.0, _collector: core::marker::PhantomData })
        }))
    }

    /// [Devicetree 3.8.1 General Properties of `/cpus/cpu*`
    /// nodes](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#general-properties-of-cpus-cpu-nodes)
    ///
    /// **Required**
    ///
    /// Specifies the current clock speed of the CPU in Hertz. The value is a
    /// `<prop-encoded-array>` in one of two forms:
    ///
    /// * A 32-bit integer consisting of one `<u32>` specifying the frequency.
    /// * A 64-bit integer represented as a `<u64>` specifying the frequency.
    #[inline]
    #[track_caller]
    pub fn clock_frequency(self) -> P::Output<u64> {
        P::to_output(crate::tryblock!({
            match self.node.properties()?.find("clock-frequency")? {
                Some(prop) => match prop.value().len() {
                    4 => Ok(u64::from(prop.as_value::<u32>()?)),
                    8 => Ok(prop.as_value::<u64>()?),
                    _ => Err(FdtError::InvalidPropertyValue),
                },
                None => {
                    let prop = self
                        .node
                        .parent()
                        .unwrap()
                        .properties()?
                        .find("clock-frequency")?
                        .ok_or(FdtError::MissingRequiredProperty("clock-frequency"))?;

                    match prop.value().len() {
                        4 => Ok(u64::from(prop.as_value::<u32>()?)),
                        8 => Ok(prop.as_value::<u64>()?),
                        _ => Err(FdtError::InvalidPropertyValue),
                    }
                }
            }
        }))
    }

    /// [Devicetree 3.8.1 General Properties of `/cpus/cpu*`
    /// nodes](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#general-properties-of-cpus-cpu-nodes)
    ///
    /// **Required**
    ///
    /// Specifies the current frequency at which the timebase and decrementer
    /// registers are updated (in Hertz). The value is a `<prop-encoded-array>`
    /// in one of two forms:
    ///
    /// * A 32-bit integer consisting of one `<u32>` specifying the frequency.
    /// * A 64-bit integer represented as a `<u64>`.
    #[inline]
    #[track_caller]
    pub fn timebase_frequency(self) -> P::Output<u64> {
        P::to_output(crate::tryblock!({
            match self.node.properties()?.find("timebase-frequency")? {
                Some(prop) => match prop.value().len() {
                    4 => Ok(u64::from(prop.as_value::<u32>()?)),
                    8 => Ok(prop.as_value::<u64>()?),
                    _ => Err(FdtError::InvalidPropertyValue),
                },
                None => {
                    let prop = self
                        .node
                        .parent()
                        .unwrap()
                        .properties()?
                        .find("timebase-frequency")?
                        .ok_or(FdtError::MissingRequiredProperty("timebase-frequency"))?;

                    match prop.value().len() {
                        4 => Ok(u64::from(prop.as_value::<u32>()?)),
                        8 => Ok(prop.as_value::<u64>()?),
                        _ => Err(FdtError::InvalidPropertyValue),
                    }
                }
            }
        }))
    }

    /// [Devicetree 3.8.1 General Properties of `/cpus/cpu*`
    /// nodes](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#general-properties-of-cpus-cpu-nodes)
    ///
    /// A standard property describing the state of a CPU. This property shall
    /// be present for nodes representing CPUs in a symmetric multiprocessing
    /// (SMP) configuration. For a CPU node the meaning of the `"okay"`,
    /// `"disabled"` and `"fail"` values are as follows:
    ///
    /// `"okay"`: The CPU is running.
    ///
    /// `"disabled"`: The CPU is in a quiescent state.
    ///
    /// `"fail"`: The CPU is not operational or does not exist.
    ///
    /// A quiescent CPU is in a state where it cannot interfere with the normal
    /// operation of other CPUs, nor can its state be affected by the normal
    /// operation of other running CPUs, except by an explicit method for
    /// enabling or re-enabling the quiescent CPU (see the enable-method
    /// property).
    ///
    /// In particular, a running CPU shall be able to issue broadcast TLB
    /// invalidates without affecting a quiescent CPU.
    ///
    /// Examples: A quiescent CPU could be in a spin loop, held in reset, and
    /// electrically isolated from the system bus or in another implementation
    /// dependent state.
    ///
    /// A CPU with `"fail"` status does not affect the system in any way. The
    /// status is assigned to nodes for which no corresponding CPU exists.
    #[inline]
    #[track_caller]
    pub fn status(&self) -> P::Output<Option<CpuStatus>> {
        P::to_output(crate::tryblock!({
            let Some(status) = self.node.properties()?.find("status")? else {
                return Ok(None);
            };

            Ok(Some(CpuStatus(status.as_value()?)))
        }))
    }

    /// [Devicetree 3.8.1 General Properties of `/cpus/cpu*`
    /// nodes](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#general-properties-of-cpus-cpu-nodes)
    ///
    /// Describes the method by which a CPU in a disabled state is enabled. This
    /// property is required for CPUs with a status property with a value of
    /// `"disabled"`. The value consists of one or more strings that define the
    /// method to release this CPU. If a client program recognizes any of the
    /// methods, it may use it. The value shall be one of the following:
    ///
    /// `"spin-table"`: The CPU is enabled with the spin table method defined in
    /// the DTSpec.
    ///
    /// `"[vendor],[method]"`: Implementation dependent string that describes
    /// the method by which a CPU is released from a `"disabled"` state. The
    /// required format is: `"[vendor],[method]"`, where vendor is a string
    /// describing the name of the manufacturer and method is a string
    /// describing the vendor specific mechanism.
    ///
    /// Example: `"fsl,MPC8572DS"`
    #[inline]
    #[track_caller]
    pub fn enable_method(&self) -> P::Output<Option<CpuEnableMethods>> {
        P::to_output(crate::tryblock!({
            let Some(status) = self.node.properties()?.find("enable-method")? else {
                return Ok(None);
            };

            let s: &'a str = status.as_value()?;

            if s.is_empty() {
                return Err(FdtError::InvalidPropertyValue);
            }

            Ok(Some(CpuEnableMethods(s.into())))
        }))
    }

    /// [Devicetree 3.8.1 General Properties of `/cpus/cpu*`
    /// nodes](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#table-10)
    ///
    /// Specifies the CPU’s MMU type.
    #[inline]
    #[track_caller]
    pub fn mmu_type(&self) -> P::Output<Option<&'a str>> {
        P::to_output(self.node.properties().and_then(|p| {
            p.find("mmu-type").and_then(|p| match p {
                Some(p) => Ok(Some(p.as_value()?)),
                None => Ok(None),
            })
        }))
    }

    /// [Devicetree 3.8.2. TLB
    /// Properties](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#tlb-properties)
    ///
    /// If present specifies that the TLB has a split configuration, with
    /// separate TLBs for instructions and data. If absent, specifies that the
    /// TLB has a unified configuration. Required for a CPU with a TLB in a
    /// split configuration.
    #[inline]
    #[track_caller]
    pub fn tlb_split(&self) -> P::Output<bool> {
        P::to_output(self.node.properties().and_then(|p| p.find("tlb-split").map(|p| p.is_some())))
    }

    /// [Devicetree 3.8.2. TLB
    /// Properties](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#tlb-properties)
    ///
    /// Specifies the number of entries in the TLB. Required for a CPU with a
    /// unified TLB for instruction and data addresses.
    #[inline]
    #[track_caller]
    pub fn tlb_size(&self) -> P::Output<Option<u32>> {
        P::to_output(self.node.properties().and_then(|p| {
            p.find("tlb-size").and_then(|p| match p {
                Some(p) => Ok(Some(p.as_value()?)),
                None => Ok(None),
            })
        }))
    }

    /// [Devicetree 3.8.2. TLB
    /// Properties](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#tlb-properties)
    ///
    /// Specifies the number of associativity sets in the TLB. Required for a
    /// CPU with a unified TLB for instruction and data addresses.
    #[inline]
    #[track_caller]
    pub fn tlb_sets(&self) -> P::Output<Option<u32>> {
        P::to_output(self.node.properties().and_then(|p| {
            p.find("tlb-sets").and_then(|p| match p {
                Some(p) => Ok(Some(p.as_value()?)),
                None => Ok(None),
            })
        }))
    }

    /// [Devicetree 3.8.2. TLB
    /// Properties](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#tlb-properties)
    ///
    /// Specifies the number of entries in the data TLB. Required for a CPU with
    /// a split TLB configuration.
    #[inline]
    #[track_caller]
    pub fn d_tlb_size(&self) -> P::Output<Option<u32>> {
        P::to_output(self.node.properties().and_then(|p| {
            p.find("d-tlb-size").and_then(|p| match p {
                Some(p) => Ok(Some(p.as_value()?)),
                None => Ok(None),
            })
        }))
    }

    /// [Devicetree 3.8.2. TLB
    /// Properties](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#tlb-properties)
    ///
    /// Specifies the number of associativity sets in the data TLB. Required for
    /// a CPU with a split TLB configuration.
    #[inline]
    #[track_caller]
    pub fn d_tlb_sets(&self) -> P::Output<Option<u32>> {
        P::to_output(self.node.properties().and_then(|p| {
            p.find("d-tlb-sets").and_then(|p| match p {
                Some(p) => Ok(Some(p.as_value()?)),
                None => Ok(None),
            })
        }))
    }

    /// [Devicetree 3.8.2. TLB
    /// Properties](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#tlb-properties)
    ///
    /// Specifies the number of entries in the instruction TLB. Required for a
    /// CPU with a split TLB configuration.
    #[inline]
    #[track_caller]
    pub fn i_tlb_size(&self) -> P::Output<Option<u32>> {
        P::to_output(self.node.properties().and_then(|p| {
            p.find("i-tlb-size").and_then(|p| match p {
                Some(p) => Ok(Some(p.as_value()?)),
                None => Ok(None),
            })
        }))
    }

    /// [Devicetree 3.8.2. TLB
    /// Properties](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#tlb-properties)
    ///
    /// Specifies the number of associativity sets in the instruction TLB.
    /// Required for a CPU with a split TLB configuration.
    #[inline]
    #[track_caller]
    pub fn i_tlb_sets(&self) -> P::Output<Option<u32>> {
        P::to_output(self.node.properties().and_then(|p| {
            p.find("i-tlb-sets").and_then(|p| match p {
                Some(p) => Ok(Some(p.as_value()?)),
                None => Ok(None),
            })
        }))
    }
}

impl<'a, P: ParserWithMode<'a>> AsNode<'a, P> for Cpu<'a, P> {
    fn as_node(&self) -> super::Node<'a, P> {
        self.node.alt()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct CpuStatus<'a>(&'a str);

impl<'a> CpuStatus<'a> {
    /// The CPU is running.
    pub const OKAY: Self = Self("okay");
    /// The CPU is in a quiescent state.
    pub const DISABLED: Self = Self("disabled");
    /// The CPU is not operational or does not exist.
    pub const FAIL: Self = Self("fail");

    /// Create a new [`CpuStatus`] which may not be one of the associated
    /// constant values.
    pub fn new(status: &'a str) -> Self {
        Self(status)
    }

    /// Whether the status is `"okay"`.
    pub fn is_okay(self) -> bool {
        self == Self::OKAY
    }

    /// Whether the status is `"disabled"`.
    pub fn is_disabled(self) -> bool {
        self == Self::DISABLED
    }

    /// Whether the status is `"failed"`
    pub fn is_failed(self) -> bool {
        self == Self::FAIL
    }
}

impl<'a> PartialEq<str> for CpuStatus<'a> {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

/// Type representing one or more CPU enable methods. See
/// [`Cpu::enable_method`].
#[derive(Debug, Clone)]
pub struct CpuEnableMethods<'a>(StringList<'a>);

impl<'a> CpuEnableMethods<'a> {
    /// Create an iterator over the enable methods.
    pub fn iter(&self) -> CpuEnableMethodsIter<'a> {
        CpuEnableMethodsIter(self.0.clone())
    }

    /// Return the first enable method contained in the list of enable methods.
    pub fn first(&self) -> CpuEnableMethod<'a> {
        self.iter().next().unwrap()
    }
}

impl<'a> IntoIterator for CpuEnableMethods<'a> {
    type IntoIter = CpuEnableMethodsIter<'a>;
    type Item = CpuEnableMethod<'a>;

    fn into_iter(self) -> Self::IntoIter {
        CpuEnableMethodsIter(self.0)
    }
}

/// Iterator over the enable methods described by the `enable-method` property
/// on a CPU node. See [`Cpu::enable_method`].
pub struct CpuEnableMethodsIter<'a>(StringList<'a>);

impl<'a> Iterator for CpuEnableMethodsIter<'a> {
    type Item = CpuEnableMethod<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next()? {
            "spin-table" => Some(CpuEnableMethod::SpinTable),
            other => {
                let (vendor, method) = other.split_once(',').unwrap_or((other, ""));
                Some(CpuEnableMethod::VendorMethod { vendor, method })
            }
        }
    }
}

/// An enable method contained by the [`Cpu::enable_method`]
pub enum CpuEnableMethod<'a> {
    /// The CPU is enabled with the spin table method defined in the DTSpec.
    SpinTable,
    /// Implementation dependent string that describes the method by which a CPU
    /// is released from a `"disabled"` state.
    VendorMethod {
        /// The manufacturer.
        vendor: &'a str,
        /// The vendor specific mechanism.
        ///
        /// NOTE: If the string value of this enable method does not match the
        /// `"[vendor],[method]"` format defined by the devicetree spec, this
        /// will be an empty string.
        method: &'a str,
    },
}

/// See [`Cpu::reg`]
pub struct CpuIds<'a, C: CellCollector> {
    reg: &'a [u8],
    address_cells: usize,
    _collector: core::marker::PhantomData<*mut C>,
}

impl<'a, C: CellCollector> CpuIds<'a, C> {
    /// The first listed CPU ID, which will always exist
    pub fn first(&self) -> Result<C::Output, CollectCellsError> {
        self.iter().next().unwrap()
    }

    pub fn iter(&self) -> CpuIdsIter<'a, C> {
        CpuIdsIter { reg: self.reg, address_cells: self.address_cells, _collector: core::marker::PhantomData }
    }
}

impl<C: CellCollector> Copy for CpuIds<'_, C> {}
impl<C: CellCollector> Clone for CpuIds<'_, C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, C: CellCollector> core::fmt::Debug for CpuIds<'a, C> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CpuIds")
            .field("reg", &self.reg)
            .field("address_cells", &self.address_cells)
            .finish_non_exhaustive()
    }
}

pub struct CpuIdsIter<'a, C: CellCollector> {
    reg: &'a [u8],
    address_cells: usize,
    _collector: core::marker::PhantomData<*mut C>,
}

impl<'a, C: CellCollector> core::fmt::Debug for CpuIdsIter<'a, C> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CpuIds")
            .field("reg", &self.reg)
            .field("address_cells", &self.address_cells)
            .finish_non_exhaustive()
    }
}

impl<C: CellCollector> Clone for CpuIdsIter<'_, C> {
    fn clone(&self) -> Self {
        Self { address_cells: self.address_cells, reg: self.reg, _collector: core::marker::PhantomData }
    }
}

impl<'a, C: CellCollector> Iterator for CpuIdsIter<'a, C> {
    type Item = Result<C::Output, CollectCellsError>;
    fn next(&mut self) -> Option<Self::Item> {
        let (this_cell, rest) = self.reg.split_at_checked(self.address_cells * 4)?;
        self.reg = rest;

        let mut collector = <C as CellCollector>::Builder::default();

        for cell in this_cell.chunks_exact(4) {
            if let Err(e) = collector.push(u32::from_be_bytes(cell.try_into().unwrap())) {
                return Some(Err(e));
            }
        }

        Some(Ok(C::map(collector.finish())))
    }
}

/// [Linux Kernel Devicetree Bindings - CPU topology binding
/// description](https://www.kernel.org/doc/Documentation/devicetree/bindings/cpu/cpu-topology.txt)
///
/// In a SMP system, the hierarchy of CPUs is defined through three entities
/// that are used to describe the layout of physical CPUs in the system:
///
/// - socket
/// - cluster
/// - core
/// - thread
///
/// The bottom hierarchy level sits at core or thread level depending on whether
/// symmetric multi-threading (SMT) is supported or not.
///
/// For instance in a system where CPUs support SMT, "cpu" nodes represent all
/// threads existing in the system and map to the hierarchy level "thread"
/// above. In systems where SMT is not supported "cpu" nodes represent all cores
/// present in the system and map to the hierarchy level "core" above.
///
/// CPU topology bindings allow one to associate cpu nodes with hierarchical
/// groups corresponding to the system hierarchy; syntactically they are defined
/// as device tree nodes.
///
/// Currently, only ARM/RISC-V intend to use this cpu topology binding but it
/// may be used for any other architecture as well.
///
/// The cpu nodes, as per bindings defined in [4][4], represent the devices that
/// correspond to physical CPUs and are to be mapped to the hierarchy levels.
///
/// A topology description containing phandles to cpu nodes that are not
/// compliant with bindings standardized in [4][4] is therefore considered invalid.
///
/// [4]: https://www.devicetree.org/specifications/
pub struct CpuTopology<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    node: FallibleNode<'a, P>,
}

impl<'a, P: ParserWithMode<'a>> CpuTopology<'a, P> {
    /// Returns an iterator over all top-level [`CpuSocket`] children. Sockets are
    /// optional for single-socket systems, so this may not return any sockets.
    /// If that is the case, iterate over the clusters instead.
    pub fn sockets(&self) -> P::Output<CpuSocketIter<'a, P>> {
        P::to_output(crate::tryblock!({
            Ok(CpuSocketIter { children: self.node.children()?.iter().filter(filter_sockets::<P>) })
        }))
    }

    /// Returns an iterator over all top-level [`CpuCluster`] children. Clusters may be
    /// contained underneath socket nodes, so if the iterator is empty, iterate
    /// over the sockets instead.
    pub fn clusters(&self) -> P::Output<CpuClusterIter<'a, P>> {
        P::to_output(crate::tryblock!({
            Ok(CpuClusterIter { children: self.node.children()?.iter().filter(filter_clusters::<P>) })
        }))
    }
}

fn filter_sockets<'a, P: ParserWithMode<'a>>(node: &Result<FallibleNode<'a, P>, FdtError>) -> bool {
    match node {
        Ok(node) => match node.name().map(|n| n.name) {
            Ok(n) if n.starts_with("socket") => true,
            _ => false,
        },
        _ => true,
    }
}

pub struct CpuSocketIter<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    children: core::iter::Filter<
        NodeChildrenIter<'a, (P::Parser, NoPanic)>,
        fn(&Result<FallibleNode<'a, P>, FdtError>) -> bool,
    >,
}

impl<'a, P: ParserWithMode<'a>> Iterator for CpuSocketIter<'a, P> {
    type Item = P::Output<CpuSocket<'a, P>>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.children.next()? {
            Ok(node) => Some(P::to_output(Ok(CpuSocket { node }))),
            Err(e) => Some(P::to_output(Err(e))),
        }
    }
}

fn filter_clusters<'a, P: ParserWithMode<'a>>(node: &Result<FallibleNode<'a, P>, FdtError>) -> bool {
    match node {
        Ok(node) => match node.name().map(|n| n.name) {
            Ok(n) if n.starts_with("cluster") => true,
            _ => false,
        },
        _ => true,
    }
}

pub struct CpuClusterIter<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    children: core::iter::Filter<
        NodeChildrenIter<'a, (P::Parser, NoPanic)>,
        fn(&Result<FallibleNode<'a, P>, FdtError>) -> bool,
    >,
}

impl<'a, P: ParserWithMode<'a>> Iterator for CpuClusterIter<'a, P> {
    type Item = P::Output<CpuCluster<'a, P>>;

    #[track_caller]
    fn next(&mut self) -> Option<Self::Item> {
        match self.children.next()? {
            Ok(node) => Some(P::to_output(Ok(CpuCluster { node }))),
            Err(e) => Some(P::to_output(Err(e))),
        }
    }
}

/// A physical CPU socket.
pub struct CpuSocket<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    node: FallibleNode<'a, P>,
}

impl<'a, P: ParserWithMode<'a>> CpuSocket<'a, P> {
    /// Returns the socket number for this particular socket, e.g. the `0` in
    /// `socket0`.
    pub fn id(&self) -> P::Output<usize> {
        P::to_output(crate::tryblock!({
            match self.node.name()?.name.trim_start_matches("socket").parse() {
                Ok(id) => Ok(id),
                Err(_) => Err(FdtError::InvalidNodeName),
            }
        }))
    }

    /// Returns an iterator over the [`CpuCluster`]s contained by this socket.
    pub fn clusters(&self) -> P::Output<CpuClusterIter<'a, P>> {
        P::to_output(crate::tryblock!({
            Ok(CpuClusterIter { children: self.node.children()?.iter().filter(filter_clusters::<P>) })
        }))
    }
}

/// A CPU cluster that is made up of either one or more clusters, or one or more [`CpuCore`]s.
pub struct CpuCluster<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    node: FallibleNode<'a, P>,
}

impl<'a, P: ParserWithMode<'a>> CpuCluster<'a, P> {
    /// Returns the cluster number for this particular cluster, e.g. the `0` in
    /// `cluster0`.
    pub fn id(&self) -> P::Output<usize> {
        P::to_output(crate::tryblock!({
            match self.node.name()?.name.trim_start_matches("cluster").parse() {
                Ok(id) => Ok(id),
                Err(_) => Err(FdtError::InvalidNodeName),
            }
        }))
    }

    /// Returns an iterator over the [`CpuCore`]s contained by this cluster.
    pub fn cores(&self) -> P::Output<CpuCoreIter<'a, P>> {
        P::to_output(crate::tryblock!({
            Ok(CpuCoreIter { children: self.node.children()?.iter().filter(filter_cores::<P>) })
        }))
    }
}

fn filter_cores<'a, P: ParserWithMode<'a>>(node: &Result<FallibleNode<'a, P>, FdtError>) -> bool {
    match node {
        Ok(node) => match node.name().map(|n| n.name) {
            Ok(n) if n.starts_with("core") => true,
            _ => false,
        },
        _ => true,
    }
}

pub struct CpuCoreIter<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    children: core::iter::Filter<
        NodeChildrenIter<'a, (P::Parser, NoPanic)>,
        fn(&Result<FallibleNode<'a, P>, FdtError>) -> bool,
    >,
}

impl<'a, P: ParserWithMode<'a>> Iterator for CpuCoreIter<'a, P> {
    type Item = P::Output<CpuCore<'a, P>>;

    #[track_caller]
    fn next(&mut self) -> Option<Self::Item> {
        match self.children.next()? {
            Ok(node) => Some(P::to_output(Ok(CpuCore { node }))),
            Err(e) => Some(P::to_output(Err(e))),
        }
    }
}

/// A physical CPU core, which may be described by a `cpu` node or a set of
/// threads if symmetric multithreading (SMT) is enabled.
pub struct CpuCore<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    node: FallibleNode<'a, P>,
}

impl<'a, P: ParserWithMode<'a>> CpuCore<'a, P> {
    /// Returns the core number for this particular core, e.g. the `0` in
    /// `core0`.
    pub fn id(&self) -> P::Output<usize> {
        P::to_output(crate::tryblock!({
            match self.node.name()?.name.trim_start_matches("core").parse() {
                Ok(id) => Ok(id),
                Err(_) => Err(FdtError::InvalidNodeName),
            }
        }))
    }

    /// If this core is described by a single physical CPU core (that is, if SMT
    /// is not enabled), return the `/cpus/cpu@N` node that represents this
    /// code. See [`Cpu`] for more details. If this returns [`None`], the core is
    /// represented by one or more [`CpuThread`]s.
    pub fn cpu(&self) -> P::Output<Option<Cpu<'a, P>>> {
        P::to_output(crate::tryblock!({
            let phandle = match self.node.properties()?.find("cpu")? {
                Some(property) => PHandle::new(property.as_value::<u32>()?),
                None => return Ok(None),
            };

            Ok(Some(Cpu {
                node: self
                    .node
                    .make_root()?
                    .resolve_phandle(phandle)?
                    .ok_or(FdtError::MissingPHandleNode(phandle.as_u32()))?,
            }))
        }))
    }

    /// Returns an iterator over all threads described by this CPU core. If this
    /// iterator does not return any [`CpuThread`]s, SMT is not enabled and the
    /// core is described by a single [`Cpu`] which can be retreived by
    /// [`CpuCore::cpu`].
    pub fn threads(&self) -> P::Output<CpuThreadIter<'a, P>> {
        P::to_output(crate::tryblock!({
            Ok(CpuThreadIter { children: self.node.children()?.iter().filter(filter_threads::<P>) })
        }))
    }
}

fn filter_threads<'a, P: ParserWithMode<'a>>(node: &Result<FallibleNode<'a, P>, FdtError>) -> bool {
    match node {
        Ok(node) => match node.name().map(|n| n.name) {
            Ok(n) if n.starts_with("thread") => true,
            _ => false,
        },
        _ => true,
    }
}

pub struct CpuThreadIter<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    children: core::iter::Filter<
        NodeChildrenIter<'a, (P::Parser, NoPanic)>,
        fn(&Result<FallibleNode<'a, P>, FdtError>) -> bool,
    >,
}

impl<'a, P: ParserWithMode<'a>> Iterator for CpuThreadIter<'a, P> {
    type Item = P::Output<CpuThread<'a, P>>;

    #[track_caller]
    fn next(&mut self) -> Option<Self::Item> {
        match self.children.next()? {
            Ok(node) => Some(P::to_output(Ok(CpuThread { node }))),
            Err(e) => Some(P::to_output(Err(e))),
        }
    }
}

/// A logical CPU thread of execution. A single [`CpuCore`] may contain multiple
/// threads if symmetric multithreading is enabled.
pub struct CpuThread<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    node: FallibleNode<'a, P>,
}

impl<'a, P: ParserWithMode<'a>> CpuThread<'a, P> {
    /// Returns the thread number for this particular thread, e.g. the `0` in
    /// `thread0`.
    pub fn id(&self) -> P::Output<usize> {
        P::to_output(crate::tryblock!({
            match self.node.name()?.name.trim_start_matches("socket").parse() {
                Ok(id) => Ok(id),
                Err(_) => Err(FdtError::InvalidNodeName),
            }
        }))
    }

    /// Returns the [`Cpu`] that is represented by this thread.
    pub fn cpu(&self) -> P::Output<Cpu<'a, P>> {
        P::to_output(crate::tryblock!({
            let phandle = match self.node.properties()?.find("cpu")? {
                Some(property) => PHandle::new(property.as_value::<u32>()?),
                None => return Err(FdtError::MissingRequiredProperty("cpu")),
            };

            self.node
                .make_root()?
                .resolve_phandle(phandle)?
                .map(|node| Cpu { node })
                .ok_or(FdtError::MissingPHandleNode(phandle.as_u32()))
        }))
    }
}
