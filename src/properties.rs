use crate::{
    nodes::Node,
    parsing::{unaligned::UnalignedParser, PanicMode, ParseError, Parser, ParserForSize},
};

pub trait Property<'a>: Sized {
    fn parse<'b, Granularity, Mode>(
        node: Node<'a, Granularity, Mode>,
    ) -> Result<Option<Self>, ParseError>
    where
        Granularity: ParserForSize,
        Mode: PanicMode;
}

pub struct CellSizes {
    pub address_cells: usize,
    pub size_cells: usize,
}

impl<'a> Property<'a> for CellSizes {
    #[track_caller]
    fn parse<'b, Granularity, Mode>(
        node: Node<'a, Granularity, Mode>,
    ) -> Result<Option<Self>, ParseError>
    where
        Granularity: ParserForSize,
        Mode: PanicMode,
    {
        let (mut address_cells, mut size_cells) = (None, None);

        for property in <Mode as PanicMode>::to_result(node.properties())? {
            let mut parser = UnalignedParser::new(property.value(), &[]);
            match property.name() {
                "#address-cells" => address_cells = Some(parser.advance_u32()?.to_ne() as usize),
                "#size-cells" => size_cells = Some(parser.advance_u32()?.to_ne() as usize),
                _ => {}
            }
        }

        if let (None, Some(parent)) = (address_cells, node.parent()) {
            address_cells =
                <Mode as PanicMode>::to_result(parent.property::<Self>())?.map(|c| c.address_cells);
        }

        if let (None, Some(parent)) = (size_cells, node.parent()) {
            size_cells =
                <Mode as PanicMode>::to_result(parent.property::<Self>())?.map(|c| c.size_cells);
        }

        Ok(Some(CellSizes {
            address_cells: address_cells.unwrap_or(2),
            size_cells: size_cells.unwrap_or(2),
        }))
    }
}
