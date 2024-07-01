use core::ffi::CStr;

use crate::{
    nodes::Node,
    parsing::{
        unaligned::UnalignedParser, BigEndianU32, NoPanic, Parser, StringsBlock, StructsBlock,
    },
    FdtError,
};

pub trait Property<'a>: Sized {
    fn parse<P: Parser<'a>>(node: Node<'a, (P, NoPanic)>) -> Result<Option<Self>, FdtError>;
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

impl<'a> Property<'a> for Compatible<'a> {
    fn parse<P: Parser<'a>>(node: Node<'a, (P, NoPanic)>) -> Result<Option<Self>, FdtError> {
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

impl<'a> Property<'a> for Model<'a> {
    fn parse<P: Parser<'a>>(node: Node<'a, (P, NoPanic)>) -> Result<Option<Self>, FdtError> {
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

impl<'a> Property<'a> for PHandle {
    fn parse<P: Parser<'a>>(node: Node<'a, (P, NoPanic)>) -> Result<Option<Self>, FdtError> {
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

impl<'a> Property<'a> for Status<'a> {
    fn parse<P: Parser<'a>>(node: Node<'a, (P, NoPanic)>) -> Result<Option<Self>, FdtError> {
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

impl<'a> Property<'a> for CellSizes {
    #[inline]
    fn parse<P: Parser<'a>>(node: Node<'a, (P, NoPanic)>) -> Result<Option<Self>, FdtError> {
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
}

impl<'a> Property<'a> for Reg<'a> {
    fn parse<P: Parser<'a>>(node: Node<'a, (P, NoPanic)>) -> Result<Option<Self>, FdtError> {
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
}
