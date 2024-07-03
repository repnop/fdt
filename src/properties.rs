use core::ffi::CStr;

use crate::{
    nodes::Node,
    parsing::{
        aligned::AlignedParser, unaligned::UnalignedParser, BigEndianU32, NoPanic, Panic,
        PanicMode, Parser, ParserWithMode, StringsBlock, StructsBlock,
    },
    standard_nodes::Root,
    FdtError,
};

pub trait Property<'a, P: Parser<'a>>: Sized {
    fn parse(
        node: Node<'a, (P, NoPanic)>,
        root: Root<'a, (P, NoPanic)>,
    ) -> Result<Option<Self>, FdtError>;
}

/// [Devicetree 2.3.1.
/// `compatible`](https://devicetree-specification.readthedocs.io/en/latest/chapter2-devicetree-basics.html#compatible)
///
/// The `compatible` property value consists of one or more strings that define
/// the specific programming model for the device. This list of strings should
/// be used by a client program for device driver selection. The property value
/// consists of a concatenated list of null terminated strings, from most
/// specific to most general. They allow a device to express its compatibility
/// with a family of similar devices, potentially allowing a single device
/// driver to match against several devices.
///
/// The recommended format is `"manufacturer,model"`, where `manufacturer` is a
/// string describing the name of the manufacturer (such as a stock ticker
/// symbol), and model specifies the model number.
///
/// The compatible string should consist only of lowercase letters, digits and
/// dashes, and should start with a letter. A single comma is typically only
/// used following a vendor prefix. Underscores should not be used.
///
/// Example: `compatible = "fsl,mpc8641", "ns16550";`
///
/// In this example, an operating system would first try to locate a device
/// driver that supported `fsl,mpc8641`. If a driver was not found, it would
/// then try to locate a driver that supported the more general `ns16550` device
/// type.
#[derive(Debug, Clone, Copy)]
pub struct Compatible<'a> {
    string: &'a str,
}

impl<'a, P: Parser<'a>> Property<'a, P> for Compatible<'a> {
    fn parse(
        node: Node<'a, (P, NoPanic)>,
        _: Root<'a, (P, NoPanic)>,
    ) -> Result<Option<Self>, FdtError> {
        let property = node.properties()?.find("compatible")?;

        match property {
            Some(prop) => Ok(Some(Self { string: prop.to()? })),
            None => Ok(None),
        }
    }
}

impl<'a> Compatible<'a> {
    /// First compatible model
    pub fn first(self) -> &'a str {
        self.string.split('\0').next().unwrap_or(self.string)
    }

    /// Returns an iterator over all compatible models
    pub fn all(self) -> CompatibleIter<'a> {
        CompatibleIter { iter: self.string.split('\0') }
    }
}

impl<'a> IntoIterator for Compatible<'a> {
    type IntoIter = CompatibleIter<'a>;
    type Item = &'a str;

    fn into_iter(self) -> Self::IntoIter {
        self.all()
    }
}

pub struct CompatibleIter<'a> {
    iter: core::str::Split<'a, char>,
}

impl<'a> Iterator for CompatibleIter<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// [Devicetree 2.3.2.
/// `model`](https://devicetree-specification.readthedocs.io/en/latest/chapter2-devicetree-basics.html#model)
///
/// The model property value is a `<string>` that specifies the manufacturer’s
/// model number of the device.
///
/// The recommended format is: `"manufacturer,model"`, where `manufacturer` is a
/// string describing the name of the manufacturer (such as a stock ticker
/// symbol), and model specifies the model number.
///
/// Example: `model = "fsl,MPC8349EMITX";`
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Model<'a>(&'a str);

impl<'a, P: Parser<'a>> Property<'a, P> for Model<'a> {
    fn parse(
        node: Node<'a, (P, NoPanic)>,
        _: Root<'a, (P, NoPanic)>,
    ) -> Result<Option<Self>, FdtError> {
        match node.properties()?.find("model")? {
            Some(model) => Ok(Some(Self(model.to()?))),
            None => Ok(None),
        }
    }
}

impl<'a> core::ops::Deref for Model<'a> {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a> core::cmp::PartialEq<str> for Model<'a> {
    fn eq(&self, other: &str) -> bool {
        self.0.eq(other)
    }
}

impl<'a> core::cmp::PartialEq<Model<'a>> for str {
    fn eq(&self, other: &Model<'a>) -> bool {
        self.eq(other.0)
    }
}

/// [Devicetree 2.3.3.
/// `phandle`](https://devicetree-specification.readthedocs.io/en/latest/chapter2-devicetree-basics.html#phandle)
///
/// The `phandle` property specifies a numerical identifier for a node that is
/// unique within the devicetree. The `phandle` property value is used by other
/// nodes that need to refer to the node associated with the property.
///
/// Example:
///
/// ```dts
/// pic@10000000 {
///    phandle = <1>;
///    interrupt-controller;
///    reg = <0x10000000 0x100>;
/// };
/// ```
///
/// A `phandle` value of `1` is defined. Another device node could reference the pic node with a `phandle` value of `1`:
///
/// ```dts
/// another-device-node {
///   interrupt-parent = <1>;
/// };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PHandle(BigEndianU32);

impl<'a, P: Parser<'a>> Property<'a, P> for PHandle {
    fn parse(
        node: Node<'a, (P, NoPanic)>,
        _: Root<'a, (P, NoPanic)>,
    ) -> Result<Option<Self>, FdtError> {
        let Some(phandle) = node.properties()?.find("phandle")? else {
            return Ok(None);
        };

        Ok(Some(PHandle(phandle.to()?)))
    }
}

/// [Devicetree 2.3.4.
/// `status`](https://devicetree-specification.readthedocs.io/en/latest/chapter2-devicetree-basics.html#status)
///
/// The `status` property indicates the operational status of a device. The lack
/// of a `status` property should be treated as if the property existed with the
/// value of `"okay"`. Valid values for the `status` property are:
///
/// | Value        | Description                                                                                                                                                                                                                                            |
/// |--------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
/// | `"okay"`     | Indicates the device is operational.                                                                                                                                                                                                                   |
/// | `"disabled"` | Indicates that the device is not presently operational, but it might become operational in the future (for example, something is not plugged in, or switched off). Refer to the device binding for details on what disabled means for a given device.  |
/// | `"reserved"` | Indicates that the device is operational, but should not be used. Typically this is used for devices that are controlled by another software component, such as platform firmware.                                                                     |
/// | `"fail"`     | Indicates that the device is not operational. A serious error was detected in the device, and it is unlikely to become operational without repair.                                                                                                     |
/// | `"fail-sss"` | Indicates that the device is not operational. A serious error was detected in the device and it is unlikely to become operational without repair. The `sss` portion of the value is specific to the device and indicates the error condition detected. |
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Status<'a>(&'a str);

impl<'a> Status<'a> {
    pub const OKAY: Self = Self("okay");
    pub const DISABLED: Self = Self("disabled");
    pub const RESERVED: Self = Self("reserved");
    pub const FAIL: Self = Self("fail");

    /// Returns true if the status is `"okay"`
    #[inline]
    pub fn is_okay(self) -> bool {
        self == Self::OKAY
    }

    /// Returns true if the status is `"disabled"`
    #[inline]
    pub fn is_disabled(self) -> bool {
        self == Self::DISABLED
    }

    /// Returns true if the status is `"reserved"`
    #[inline]
    pub fn is_reserved(self) -> bool {
        self == Self::RESERVED
    }

    /// Returns true if the status is `"fail"` or begins with `"fail-"`
    #[inline]
    pub fn is_failed(self) -> bool {
        self == Self::FAIL || self.0.starts_with("fail-")
    }

    /// Returns the `sss` portion of the `fail-sss` status, if the status is
    /// failed and contains a status condition
    pub fn failed_status_code(self) -> Option<&'a str> {
        match self.0.starts_with("fail-") {
            true => Some(self.0.trim_start_matches("fail-")),
            false => None,
        }
    }
}

impl<'a, P: Parser<'a>> Property<'a, P> for Status<'a> {
    fn parse(
        node: Node<'a, (P, NoPanic)>,
        _: Root<'a, (P, NoPanic)>,
    ) -> Result<Option<Self>, FdtError> {
        match node.properties()?.find("status")? {
            Some(model) => Ok(Some(Self(model.to()?))),
            None => Ok(None),
        }
    }
}

impl<'a> core::ops::Deref for Status<'a> {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a> core::cmp::PartialEq<str> for Status<'a> {
    fn eq(&self, other: &str) -> bool {
        self.0.eq(other)
    }
}

impl<'a> core::cmp::PartialEq<Status<'a>> for str {
    fn eq(&self, other: &Status<'a>) -> bool {
        self.eq(other.0)
    }
}

/// [Devicetree 2.3.5. `#address-cells` and
/// `#size-cells`](https://devicetree-specification.readthedocs.io/en/latest/chapter2-devicetree-basics.html#address-cells-and-size-cells)
///
/// The `#address-cells` and `#size-cells` properties may be used in any device
/// node that has children in the devicetree hierarchy and describes how child
/// device nodes should be addressed. The `#address-cells` property defines the
/// number of `<u32>` cells used to encode the address field in a child node’s
/// reg property. The `#size-cells` property defines the number of `<u32>` cells
/// used to encode the size field in a child node’s reg property.
///
/// The `#address-cells` and `#size-cells` properties are not inherited from
/// ancestors in the devicetree. They shall be explicitly defined.
///
/// A DTSpec-compliant boot program shall supply `#address-cells` and
/// `#size-cells` on all nodes that have children.
///
/// If missing, a client program should assume a default value of 2 for
/// `#address-cells`, and a value of 1 for `#size-cells`.
///
/// Example:
///
/// ```dts
/// soc {
///    #address-cells = <1>;
///    #size-cells = <1>;
///
///    serial@4600 {
///       compatible = "ns16550";
///       reg = <0x4600 0x100>;
///       clock-frequency = <0>;
///       interrupts = <0xA 0x8>;
///       interrupt-parent = <&ipic>;
///    };
/// };
/// ```
///
/// In this example, the `#address-cells` and `#size-cells` properties of the
/// soc node are both set to `1`. This setting specifies that one cell is
/// required to represent an address and one cell is required to represent the
/// size of nodes that are children of this node.
///
/// The serial device reg property necessarily follows this specification set in
/// the parent (soc) node—the address is represented by a single cell
/// (`0x4600`), and the size is represented by a single cell (`0x100`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellSizes {
    pub address_cells: usize,
    pub size_cells: usize,
}

impl<'a, P: Parser<'a>> Property<'a, P> for CellSizes {
    #[inline]
    fn parse(
        node: Node<'a, (P, NoPanic)>,
        _: Root<'a, (P, NoPanic)>,
    ) -> Result<Option<Self>, FdtError> {
        let (mut address_cells, mut size_cells) = (None, None);

        for property in node.properties()? {
            let property = property?;

            let mut parser =
                UnalignedParser::new(property.value(), StringsBlock(&[]), StructsBlock(&[]));
            match property.name() {
                "#address-cells" => address_cells = Some(parser.advance_u32()?.to_ne() as usize),
                "#size-cells" => size_cells = Some(parser.advance_u32()?.to_ne() as usize),
                _ => {}
            }
        }

        Ok(address_cells
            .zip(size_cells)
            .map(|(address_cells, size_cells)| CellSizes { address_cells, size_cells }))
    }
}

impl Default for CellSizes {
    fn default() -> Self {
        CellSizes { address_cells: 2, size_cells: 1 }
    }
}

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

    pub fn iter_u64(self) -> RegU64Iter<'a> {
        RegU64Iter { cell_sizes: self.cell_sizes, encoded_array: self.encoded_array }
    }
}

impl<'a, P: Parser<'a>> Property<'a, P> for Reg<'a> {
    fn parse(
        node: Node<'a, (P, NoPanic)>,
        _: Root<'a, (P, NoPanic)>,
    ) -> Result<Option<Self>, FdtError> {
        let Some(prop) = node.raw_property("reg")? else {
            return Ok(None);
        };

        let cell_sizes = match node.parent() {
            Some(parent) => parent.property::<CellSizes>()?.unwrap_or_default(),
            None => CellSizes::default(),
        };

        let encoded_array = prop.value();

        if encoded_array.len() % (cell_sizes.address_cells * 4 + cell_sizes.size_cells * 4) != 0 {
            return Err(FdtError::InvalidPropertyValue);
        }

        Ok(Some(Self { cell_sizes, encoded_array }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegEntry<I> {
    pub address: I,
    pub len: I,
}

pub struct RegU64Iter<'a> {
    cell_sizes: CellSizes,
    encoded_array: &'a [u8],
}

impl<'a> Iterator for RegU64Iter<'a> {
    type Item = RegEntry<u64>;
    fn next(&mut self) -> Option<Self::Item> {
        let address_bytes = self.cell_sizes.address_cells * 4;
        let size_bytes = self.cell_sizes.size_cells * 4;

        let encoded_address = self.encoded_array.get(..address_bytes)?;
        let encoded_len = self.encoded_array.get(address_bytes..address_bytes + size_bytes)?;

        let mut address: u64 = 0;
        let mut len: u64 = 0;

        for encoded_address in encoded_address.chunks_exact(4) {
            address = address.wrapping_shl(32);

            // TODO: replace this stuff with `array_chunks` when its stabilized
            //
            // These unwraps can't panic because `chunks_exact` guarantees that
            // we'll always get slices of 4 bytes
            let addr = u64::from(u32::from_be_bytes(encoded_address.try_into().unwrap()));
            address = address.wrapping_add(addr);
        }

        for encoded_len in encoded_len.chunks_exact(4) {
            len = len.wrapping_shl(32);

            // TODO: replace this stuff with `array_chunks` when its stabilized
            //
            // These unwraps can't panic because `chunks_exact` guarantees that
            // we'll always get slices of 4 bytes
            let l = u64::from(u32::from_be_bytes(encoded_len.try_into().unwrap()));
            len = len.wrapping_add(l);
        }

        self.encoded_array = self.encoded_array.get((address_bytes + size_bytes)..)?;
        Some(RegEntry { address, len })
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

        let address = self.encoded_array.get(..address_bytes)?;
        let len = self.encoded_array.get(address_bytes..address_bytes + size_bytes)?;
        self.encoded_array = self.encoded_array.get((address_bytes + size_bytes)..)?;
        Some(RawRegEntry { address, len })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawRegEntry<'a> {
    address: &'a [u8],
    len: &'a [u8],
}

impl<'a> RawRegEntry<'a> {
    pub fn address(self) -> &'a [u8] {
        self.address
    }

    pub fn len(self) -> &'a [u8] {
        self.len
    }
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

impl<'a, P: Parser<'a>> Property<'a, P> for VirtualReg {
    fn parse(
        node: Node<'a, (P, NoPanic)>,
        _: Root<'a, (P, NoPanic)>,
    ) -> Result<Option<Self>, FdtError> {
        match node.properties()?.find("virtual-reg")? {
            Some(vreg) => Ok(Some(Self(vreg.to()?))),
            None => Ok(None),
        }
    }
}

pub struct Ranges<'a> {
    cell_sizes: CellSizes,
    ranges: &'a [u8],
}

/// [Devicetree 2.3.10.
/// `dma-coherent`](https://devicetree-specification.readthedocs.io/en/latest/chapter2-devicetree-basics.html#dma-coherent)
///
/// For architectures which are by default non-coherent for I/O, the
/// `dma-coherent` property is used to indicate a device is capable of coherent
/// DMA operations. Some architectures have coherent DMA by default and this
/// property is not applicable.
pub struct DmaCoherent;

impl<'a, P: Parser<'a>> Property<'a, P> for DmaCoherent {
    fn parse(
        node: Node<'a, (P, NoPanic)>,
        _: Root<'a, (P, NoPanic)>,
    ) -> Result<Option<Self>, FdtError> {
        match node.properties()?.find("dma-coherent")? {
            Some(_) => Ok(Some(Self)),
            None => Ok(None),
        }
    }
}

/// Enum representing the two possibilities for interrupt descriptions on a
/// devicetree node. See the documentation for each type for more information.
/// [`ExtendedInterrupts`] will take precedence if both properties exist.
pub enum Interrupts<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    Legacy(LegacyInterrupts<'a, P>),
    Extended(ExtendedInterrupts<'a, P>),
}

impl<'a, P: Parser<'a>, Mode: PanicMode + Clone + Default + 'static> Property<'a, P>
    for Interrupts<'a, (P, Mode)>
{
    fn parse(
        node: Node<'a, (P, NoPanic)>,
        root: Root<'a, (P, NoPanic)>,
    ) -> Result<Option<Self>, FdtError> {
        match ExtendedInterrupts::parse(node, root)? {
            Some(extended) => Ok(Some(Self::Extended(extended))),
            None => match LegacyInterrupts::parse(node, root)? {
                Some(legacy) => Ok(Some(Self::Legacy(legacy))),
                None => Ok(None),
            },
        }
    }
}

/// [Devicetree 2.4.1.1.
/// `interrupts`](https://devicetree-specification.readthedocs.io/en/latest/chapter2-devicetree-basics.html#interrupts)
///
/// The `interrupts` property of a device node defines the interrupt or
/// interrupts that are generated by the device. The value of the `interrupts`
/// property consists of an arbitrary number of interrupt specifiers. The format
/// of an interrupt specifier is defined by the binding of the interrupt domain
/// root.
///
/// `interrupts` is overridden by the `interrupts-extended` property and
/// normally only one or the other should be used.
pub struct LegacyInterrupts<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    interrupt_parent: InterruptParent<'a, P>,
    interrupt_cells: InterruptCells,
    encoded_array: &'a [u8],
}

impl<'a, P: Parser<'a>, Mode: PanicMode + Clone + Default + 'static> Property<'a, P>
    for LegacyInterrupts<'a, (P, Mode)>
{
    fn parse(
        node: Node<'a, (P, NoPanic)>,
        root: Root<'a, (P, NoPanic)>,
    ) -> Result<Option<Self>, FdtError> {
        match node.properties()?.find("interrupts")? {
            Some(interrupts) => {
                let interrupt_parent = match InterruptParent::<'a, (P, NoPanic)>::parse(node, root)?
                {
                    Some(p) => p,
                    None => return Err(FdtError::MissingRequiredProperty("interrupt-parent")),
                };

                let Some(interrupt_cells) = interrupt_parent.property::<InterruptCells>()? else {
                    return Err(FdtError::MissingRequiredProperty("interrupt-cells"));
                };

                if interrupts.value().len() % (interrupt_cells.0 * 4) as usize != 0 {
                    return Err(FdtError::InvalidPropertyValue);
                }

                Ok(Some(Self {
                    interrupt_parent: InterruptParent(interrupt_parent.0.alt()),
                    interrupt_cells,
                    encoded_array: interrupts.value(),
                }))
            }
            None => Ok(None),
        }
    }
}

/// [Devicetree 2.4.1.3.
/// `interrupts-extended`](https://devicetree-specification.readthedocs.io/en/latest/chapter2-devicetree-basics.html#interrupts-extended)
///
/// The `interrupts-extended` property lists the interrupt(s) generated by a
/// device. `interrupts-extended` should be used instead of interrupts when a
/// device is connected to multiple interrupt controllers as it encodes a parent
/// `phandle` with each interrupt specifier.
///
/// Example:
///
/// This example shows how a device with two interrupt outputs connected to two
/// separate interrupt controllers would describe the connection using an
/// `interrupts-extended` property. `pic` is an interrupt controller with an
/// `#interrupt-cells` specifier of 2, while `gic` is an interrupt controller
/// with an `#interrupts-cells` specifier of 1.
///
/// `interrupts-extended = <&pic 0xA 8>, <&gic 0xda>;`
pub struct ExtendedInterrupts<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    root: Root<'a, P>,
    encoded_array: &'a [u8],
}

impl<'a, P: ParserWithMode<'a>> ExtendedInterrupts<'a, P> {
    pub fn iter(self) -> ExtendedInterruptsIter<'a, P> {
        ExtendedInterruptsIter { root: self.root, encoded_array: self.encoded_array }
    }
}

impl<'a, P: Parser<'a>, Mode: PanicMode + Clone + Default + 'static> Property<'a, P>
    for ExtendedInterrupts<'a, (P, Mode)>
{
    fn parse(
        node: Node<'a, (P, NoPanic)>,
        root: Root<'a, (P, NoPanic)>,
    ) -> Result<Option<Self>, FdtError> {
        match node.properties()?.find("interrupts-extended")? {
            Some(interrupts) => Ok(Some(Self {
                encoded_array: interrupts.value(),
                root: Root { node: root.node.alt() },
            })),

            None => Ok(None),
        }
    }
}

pub struct ExtendedInterruptsIter<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    root: Root<'a, P>,
    encoded_array: &'a [u8],
}

impl<'a, P: ParserWithMode<'a>> Iterator for ExtendedInterruptsIter<'a, P> {
    type Item = P::Output<ExtendedInterrupt<'a, P>>;

    #[track_caller]
    fn next(&mut self) -> Option<Self::Item> {
        let phandle = self.encoded_array.get(..4).map(|bytes| {
            PHandle(BigEndianU32::from_be(u32::from_ne_bytes(bytes.try_into().unwrap())))
        })?;
        self.encoded_array = self.encoded_array.get(4..)?;

        let res = crate::tryblock! {
            let root = Root { node: self.root.node.fallible() };
            let Some(interrupt_parent) = root.resolve_phandle(phandle)? else {
                return Err(FdtError::PHandleNotFound(phandle.0.to_ne()));
            };

            let Some(interrupt_cells) = interrupt_parent.property::<InterruptCells>()? else {
                return Err(FdtError::MissingRequiredProperty("#interrupt-cells"));
            };

            let cells_length = 4 * interrupt_cells.0 as usize;
            let encoded_array = match self.encoded_array.get(..cells_length) {
                Some(bytes) => bytes,
                None => return Ok(None),
            };

            self.encoded_array = match self.encoded_array.get(cells_length..) {
                Some(bytes) => bytes,
                None => return Ok(None),
            };

            Ok(Some(ExtendedInterrupt {
                interrupt_parent: InterruptParent(interrupt_parent.alt()),
                interrupt_cells,
                encoded_array,
            }))
        };

        // This is a manual impl of `map` because we need the panic location to
        // be the caller if `P::to_output` panics
        #[allow(clippy::manual_map)]
        match res.transpose() {
            Some(output) => Some(P::to_output(output)),
            None => None,
        }
    }
}

/// A single entry in an `interrupts-extended` property
pub struct ExtendedInterrupt<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    interrupt_parent: InterruptParent<'a, P>,
    interrupt_cells: InterruptCells,
    encoded_array: &'a [u8],
}

impl<'a, P: ParserWithMode<'a>> ExtendedInterrupt<'a, P> {
    pub fn interrupt_parent(self) -> InterruptParent<'a, P> {
        self.interrupt_parent
    }

    pub fn interrupt_cells(self) -> InterruptCells {
        self.interrupt_cells
    }

    pub fn interrupt_specifier(self) -> InterruptSpecifier<'a> {
        InterruptSpecifier {
            interrupt_cells: self.interrupt_cells,
            encoded_array: self.encoded_array,
        }
    }
}

pub struct InterruptSpecifier<'a> {
    interrupt_cells: InterruptCells,
    encoded_array: &'a [u8],
}

impl<'a> InterruptSpecifier<'a> {
    /// Iterate over the components that comprise this interrupt specifier
    pub fn iter(self) -> InterruptSpecifierIter<'a> {
        InterruptSpecifierIter { encoded_array: self.encoded_array }
    }

    /// Extract the single component that comprises the interrupt specifier, if
    /// the `#interrupt-cells` value is `1`
    pub fn single(self) -> Option<u32> {
        if self.interrupt_cells.0 != 1 {
            return None;
        }

        self.iter().next()
    }

    /// Extract the two components that comprise the interrupt specifier, if the
    /// `#interrupt-cells` value is `2`
    pub fn pair(self) -> Option<(u32, u32)> {
        if self.interrupt_cells.0 != 2 {
            return None;
        }

        let mut iter = self.into_iter();
        Some((iter.next()?, iter.next()?))
    }
}

impl<'a> IntoIterator for InterruptSpecifier<'a> {
    type IntoIter = InterruptSpecifierIter<'a>;
    type Item = u32;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over individual components in an interrupt specifier
pub struct InterruptSpecifierIter<'a> {
    encoded_array: &'a [u8],
}

impl<'a> Iterator for InterruptSpecifierIter<'a> {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.encoded_array.is_empty() {
            return None;
        }

        let next = self.encoded_array.get(..4)?;
        self.encoded_array = self.encoded_array.get(4..)?;

        // This panic can never fail since the slice length is guaranteed to be
        // 4 bytes long
        Some(u32::from_be_bytes(next.try_into().unwrap()))
    }
}

/// Iterator over pairs of `u32`s representing an interrupt specifier
pub struct InterruptSpecifierIterPairs<'a> {
    encoded_array: &'a [u8],
}

impl<'a> Iterator for InterruptSpecifierIterPairs<'a> {
    type Item = (u32, u32);

    fn next(&mut self) -> Option<Self::Item> {
        if self.encoded_array.is_empty() {
            return None;
        }

        let next = self.encoded_array.get(..8)?;
        self.encoded_array = self.encoded_array.get(8..)?;

        // This panic can never fail since the slice length is guaranteed to be
        // 4 bytes long
        Some((
            u32::from_be_bytes(next[..4].try_into().unwrap()),
            u32::from_be_bytes(next[4..8].try_into().unwrap()),
        ))
    }
}

/// [Devicetree 2.4.1.2.
/// `interrupt-parent`](https://devicetree-specification.readthedocs.io/en/latest/chapter2-devicetree-basics.html#interrupt-parent)
///
/// Because the hierarchy of the nodes in the interrupt tree might not match the
/// devicetree, the `interrupt-parent` property is available to make the
/// definition of an interrupt parent explicit. The value is the `phandle` to
/// the interrupt parent. If this property is missing from a device, its
/// interrupt parent is assumed to be its devicetree parent.
pub struct InterruptParent<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)>(Node<'a, P>);

impl<'a, P: ParserWithMode<'a>> core::ops::Deref for InterruptParent<'a, P> {
    type Target = Node<'a, P>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'a, P: ParserWithMode<'a>> core::ops::DerefMut for InterruptParent<'a, P> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'a, P: Parser<'a>, Mode: PanicMode + Clone + Default + 'static> Property<'a, P>
    for InterruptParent<'a, (P, Mode)>
{
    fn parse(
        node: Node<'a, (P, NoPanic)>,
        root: Root<'a, (P, NoPanic)>,
    ) -> Result<Option<Self>, FdtError> {
        match node.properties()?.find("interrupt-parent")? {
            Some(phandle) => match root.resolve_phandle(PHandle(phandle.to()?))? {
                Some(parent) => Ok(Some(Self(parent.alt()))),
                None => Err(FdtError::PHandleNotFound(phandle.to()?)),
            },
            None => Ok(node.parent().map(|n| Self(n.alt()))),
        }
    }
}

/// [Devicetree 2.4.2.1.
/// `#interrupt-cells`](https://devicetree-specification.readthedocs.io/en/latest/chapter2-devicetree-basics.html#interrupt-cells)
///
/// The `#interrupt-cells` property defines the number of cells required to
/// encode an interrupt specifier for an interrupt domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InterruptCells(u32);

impl InterruptCells {
    pub fn into_u32(self) -> u32 {
        self.0
    }
}

impl<'a, P: Parser<'a>> Property<'a, P> for InterruptCells {
    fn parse(
        node: Node<'a, (P, NoPanic)>,
        _: Root<'a, (P, NoPanic)>,
    ) -> Result<Option<Self>, FdtError> {
        match node.properties()?.find("#interrupt-cells")? {
            Some(ic) => Ok(Some(Self(ic.to()?))),
            None => Ok(None),
        }
    }
}

/// [Devicetree 2.4.2.2.
/// `interrupt-controller`](https://devicetree-specification.readthedocs.io/en/latest/chapter2-devicetree-basics.html#interrupt-controller)
///
/// The presence of an `interrupt-controller` property defines a node as an
/// interrupt controller node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InterruptController;

impl<'a, P: Parser<'a>> Property<'a, P> for InterruptController {
    fn parse(
        node: Node<'a, (P, NoPanic)>,
        _: Root<'a, (P, NoPanic)>,
    ) -> Result<Option<Self>, FdtError> {
        match node.properties()?.find("interrupt-controller")? {
            Some(_) => Ok(Some(Self)),
            None => Ok(None),
        }
    }
}

pub struct InvalidPropertyValue;

impl From<InvalidPropertyValue> for FdtError {
    fn from(_: InvalidPropertyValue) -> Self {
        FdtError::InvalidPropertyValue
    }
}

pub trait PropertyValue<'a>: Sized {
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue>;
}

impl<'a> PropertyValue<'a> for u32 {
    #[inline]
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue> {
        match value {
            [a, b, c, d] => Ok(u32::from_be_bytes([*a, *b, *c, *d])),
            _ => Err(InvalidPropertyValue),
        }
    }
}

impl<'a> PropertyValue<'a> for BigEndianU32 {
    #[inline]
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue> {
        match value {
            [a, b, c, d] => Ok(BigEndianU32::from_be(u32::from_ne_bytes([*a, *b, *c, *d]))),
            _ => Err(InvalidPropertyValue),
        }
    }
}

impl<'a> PropertyValue<'a> for &'a CStr {
    #[inline]
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue> {
        CStr::from_bytes_until_nul(value).map_err(|_| InvalidPropertyValue)
    }
}

impl<'a> PropertyValue<'a> for &'a str {
    #[inline]
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue> {
        core::str::from_utf8(value)
            .map(|s| s.trim_end_matches('\0'))
            .map_err(|_| InvalidPropertyValue)
    }
}

#[derive(Debug, Clone)]
pub struct StringList<'a> {
    strs: core::str::Split<'a, char>,
}

impl<'a> PropertyValue<'a> for StringList<'a> {
    #[inline]
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue> {
        Ok(Self { strs: <&'a str as PropertyValue<'a>>::parse(value)?.split('\0') })
    }
}

impl<'a> Iterator for StringList<'a> {
    type Item = &'a str;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.strs.next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reg_raw_iter() {
        let mut iter = RegRawIter {
            cell_sizes: CellSizes { address_cells: 2, size_cells: 1 },
            encoded_array: &[
                0x55, 0x44, 0x33, 0x22, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            ],
        };

        assert_eq!(
            iter.next().unwrap(),
            RawRegEntry {
                address: &[0x55, 0x44, 0x33, 0x22, 0x66, 0x77, 0x88, 0x99],
                len: &[0xAA, 0xBB, 0xCC, 0xDD]
            }
        );
    }

    #[test]
    fn reg_u64_iter() {
        let mut iter = RegU64Iter {
            cell_sizes: CellSizes { address_cells: 2, size_cells: 1 },
            encoded_array: &[
                0x55, 0x44, 0x33, 0x22, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            ],
        };

        assert_eq!(iter.next().unwrap(), RegEntry { address: 0x5544332266778899, len: 0xAABBCCDD });
    }
}
