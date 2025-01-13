// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod cells;
pub mod interrupts;
pub mod ranges;
pub mod reg;
pub mod values;

use crate::{
    helpers::{FallibleNode, FallibleRoot},
    parsing::{BigEndianU32, ParserWithMode},
    FdtError,
};

/// A property (or potentially a group of related properties, see
/// [`cells::CellSizes`]) that can be parsed from a [`crate::nodes::Node`] which
/// may also need additional information from the devicetree.
pub trait Property<'a, P: ParserWithMode<'a>>: Sized {
    fn parse(node: FallibleNode<'a, P>, root: FallibleRoot<'a, P>) -> Result<Option<Self>, FdtError>;
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

impl<'a, P: ParserWithMode<'a>> Property<'a, P> for Compatible<'a> {
    fn parse(node: FallibleNode<'a, P>, _: FallibleRoot<'a, P>) -> Result<Option<Self>, FdtError> {
        let property = node.properties()?.find("compatible")?;

        match property {
            Some(prop) => Ok(Some(Self { string: prop.as_value()? })),
            None => Ok(None),
        }
    }
}

impl<'a> Compatible<'a> {
    /// First compatible model.
    pub fn first(self) -> &'a str {
        self.string.split('\0').next().unwrap_or(self.string)
    }

    /// Returns an iterator over all compatible models.
    pub fn all(self) -> CompatibleIter<'a> {
        CompatibleIter { iter: self.string.split('\0') }
    }

    /// Returns whether the node is compatible with the given string value.
    pub fn compatible_with(self, value: &str) -> bool {
        self.all().any(|c| c == value)
    }
}

impl<'a> IntoIterator for Compatible<'a> {
    type IntoIter = CompatibleIter<'a>;
    type Item = &'a str;

    fn into_iter(self) -> Self::IntoIter {
        self.all()
    }
}

/// An iterator over all of the strings contained within a [`Compatible`]
/// property.
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

impl<'a, P: ParserWithMode<'a>> Property<'a, P> for Model<'a> {
    fn parse(node: FallibleNode<'a, P>, _: FallibleRoot<'a, P>) -> Result<Option<Self>, FdtError> {
        match node.properties()?.find("model")? {
            Some(model) => Ok(Some(Self(model.as_value()?))),
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

impl<'a> AsRef<str> for Model<'a> {
    fn as_ref(&self) -> &str {
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

impl PHandle {
    /// Create a new [`PHandle`] using an existing handle ID.
    ///
    /// Note: this function will convert the handle ID to big endian, so it
    /// should be in the platform's native endianness.
    pub fn new(handle: u32) -> Self {
        Self(BigEndianU32::from_ne(handle))
    }

    /// Return the [`PHandle`]'s value as a native-endianness [`u32`].
    pub fn as_u32(self) -> u32 {
        self.0.to_ne()
    }
}

impl<'a, P: ParserWithMode<'a>> Property<'a, P> for PHandle {
    fn parse(node: FallibleNode<'a, P>, _: FallibleRoot<'a, P>) -> Result<Option<Self>, FdtError> {
        let Some(phandle) = node.properties()?.find("phandle")? else {
            return Ok(None);
        };

        Ok(Some(PHandle(phandle.as_value()?)))
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

impl<'a, P: ParserWithMode<'a>> Property<'a, P> for Status<'a> {
    fn parse(node: FallibleNode<'a, P>, _: FallibleRoot<'a, P>) -> Result<Option<Self>, FdtError> {
        match node.properties()?.find("status")? {
            Some(model) => Ok(Some(Self(model.as_value()?))),
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

/// [Devicetree 2.3.10.
/// `dma-coherent`](https://devicetree-specification.readthedocs.io/en/latest/chapter2-devicetree-basics.html#dma-coherent)
///
/// For architectures which are by default non-coherent for I/O, the
/// `dma-coherent` property is used to indicate a device is capable of coherent
/// DMA operations. Some architectures have coherent DMA by default and this
/// property is not applicable.
pub struct DmaCoherent;

impl<'a, P: ParserWithMode<'a>> Property<'a, P> for DmaCoherent {
    fn parse(node: FallibleNode<'a, P>, _: FallibleRoot<'a, P>) -> Result<Option<Self>, FdtError> {
        match node.properties()?.find("dma-coherent")? {
            Some(_) => Ok(Some(Self)),
            None => Ok(None),
        }
    }
}
