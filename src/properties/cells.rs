use crate::{
    nodes::{FallibleNode, FallibleRoot},
    parsing::{unaligned::UnalignedParser, Parser, ParserWithMode, StringsBlock, StructsBlock},
    FdtError,
};

use super::Property;

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

impl<'a, P: ParserWithMode<'a>> Property<'a, P> for CellSizes {
    #[inline]
    fn parse(node: FallibleNode<'a, P>, _: FallibleRoot<'a, P>) -> Result<Option<Self>, FdtError> {
        let (mut address_cells, mut size_cells) = (None, None);

        for property in node.properties()? {
            let property = property?;

            let mut parser = UnalignedParser::new(property.value(), StringsBlock(&[]), StructsBlock(&[]));
            match property.name() {
                "#address-cells" => address_cells = Some(parser.advance_u32()?.to_ne() as usize),
                "#size-cells" => size_cells = Some(parser.advance_u32()?.to_ne() as usize),
                _ => {}
            }
        }

        Ok(address_cells.zip(size_cells).map(|(address_cells, size_cells)| CellSizes { address_cells, size_cells }))
    }
}

impl Default for CellSizes {
    fn default() -> Self {
        CellSizes { address_cells: 2, size_cells: 1 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddressCells(pub usize);

impl<'a, P: ParserWithMode<'a>> Property<'a, P> for AddressCells {
    #[inline]
    fn parse(node: FallibleNode<'a, P>, _: FallibleRoot<'a, P>) -> Result<Option<Self>, FdtError> {
        match node.properties()?.find("#address-cells")? {
            Some(value) => Ok(Some(Self(value.as_value()?))),
            None => Ok(None),
        }
    }
}

impl Default for AddressCells {
    fn default() -> Self {
        Self(2)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SizeCells(pub usize);

impl<'a, P: ParserWithMode<'a>> Property<'a, P> for SizeCells {
    #[inline]
    fn parse(node: FallibleNode<'a, P>, _: FallibleRoot<'a, P>) -> Result<Option<Self>, FdtError> {
        match node.properties()?.find("#size-cells")? {
            Some(value) => Ok(Some(Self(value.as_value()?))),
            None => Ok(None),
        }
    }
}

impl Default for SizeCells {
    fn default() -> Self {
        Self(1)
    }
}
