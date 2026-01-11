// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    cell_collector::CollectCellsError,
    nodes::{root::Root, Node, NodeName},
    parsing::{NoPanic, Parser},
    properties::values::{InvalidPropertyValue, U32List},
    FdtError,
};

#[derive(Debug)]
pub struct Error;

impl From<FdtError> for Error {
    #[track_caller]
    fn from(_: FdtError) -> Self {
        Error
    }
}

impl From<CollectCellsError> for Error {
    #[track_caller]
    fn from(_: CollectCellsError) -> Self {
        Error
    }
}

impl From<InvalidPropertyValue> for Error {
    #[track_caller]
    fn from(_: InvalidPropertyValue) -> Self {
        Error
    }
}

impl From<core::fmt::Error> for Error {
    fn from(_: core::fmt::Error) -> Self {
        Error
    }
}

impl From<Error> for core::fmt::Error {
    fn from(_: Error) -> Self {
        core::fmt::Error
    }
}

pub fn print_fdt<'a, P: Parser<'a>>(
    f: &mut core::fmt::Formatter<'_>,
    root: Root<'a, (P, NoPanic)>,
) -> core::fmt::Result {
    let res = crate::tryblock!(Error, {
        let mut any_children = true;
        let mut any_props;
        let mut node_iter = root.all_nodes()?.peekable();
        let (mut n_braces, mut final_depth) = (0, 0);
        writeln!(f, "/ {{")?;
        any_props = print_properties(f, root.node, 0)?;
        while let Some((depth, node)) = node_iter.next().transpose()? {
            let next_depth = match node_iter.peek().cloned().transpose()? {
                Some((next_depth, _)) => next_depth,
                None => 0,
            };
            let next_is_child = next_depth > depth;
            any_children = true;

            if n_braces > 0 {
                for _ in (0..n_braces).rev() {
                    final_depth -= 1;
                    writeln!(f, "{:width$}}};", ' ', width = (final_depth) * 4)?;
                }

                n_braces = 0;
            }

            if any_props {
                writeln!(f)?;
            }

            writeln!(
                f,
                "{:width$}{} {{",
                ' ',
                if node.name()?.name.is_empty() { NodeName { name: "/", unit_address: None } } else { node.name()? },
                width = depth * 4,
            )?;

            any_props = print_properties(f, node, depth)?;

            if !any_props && !next_is_child {
                writeln!(f)?;
            }

            if next_depth <= depth {
                writeln!(f, "{:width$}}};", ' ', width = depth * 4)?;

                if !any_props && !next_is_child {
                    writeln!(f)?;
                }
            }

            if depth > next_depth {
                n_braces += depth - next_depth;
                final_depth = depth;
            } else {
                n_braces = 0;
                final_depth = 0;
            }
        }

        if any_children {
            writeln!(f, "    }};")?;
        }

        write!(f, "}};")?;

        Ok(())
    });

    Ok(res?)
}

fn print_properties<'a, P: Parser<'a>>(
    f: &mut core::fmt::Formatter<'_>,
    node: Node<'a, (P, NoPanic)>,
    depth: usize,
) -> Result<bool, Error> {
    let mut any_props = false;
    for prop in node.properties()? {
        any_props = true;
        let prop = prop?;

        match prop.name {
            "reg" => {
                write!(f, "{:width$}reg = <", ' ', width = depth * 4 + 4)?;
                for (i, reg) in node.reg()?.unwrap().iter::<u64, Option<u64>>().enumerate() {
                    let reg = reg?;
                    if i > 0 {
                        write!(f, " ")?;
                    }

                    match reg.len {
                        Some(size) => write!(f, "{:#04x} {:#04x}", reg.address as usize, size)?,
                        None => write!(f, "{:#04x}", reg.address as usize)?,
                    }
                }
                writeln!(f, ">;")?;
            }
            "compatible" => {
                writeln!(f, "{:width$}compatible = {:?};", ' ', prop.as_value::<&str>()?, width = depth * 4 + 4)?
            }
            name if name.contains("-cells") => {
                writeln!(f, "{:width$}{} = <{:#04x}>;", ' ', name, prop.as_value::<u32>()?, width = depth * 4 + 4)?;
            }
            _ => match prop.as_value::<&str>() {
                Ok("") => writeln!(f, "{:width$}{};", ' ', prop.name, width = depth * 4 + 4)?,
                Ok(value) => writeln!(f, "{:width$}{} = {:?};", ' ', prop.name, value, width = depth * 4 + 4)?,
                _ => match prop.value.len() {
                    0 => writeln!(f, "{:width$}{};", ' ', prop.name, width = depth * 4 + 4)?,
                    _ => {
                        write!(f, "{:width$}{} = <", ' ', prop.name, width = depth * 4 + 4)?;

                        for (i, n) in prop.as_value::<U32List>()?.iter().enumerate() {
                            if i != 0 {
                                write!(f, " ")?;
                            }

                            write!(f, "{n:#04x}")?;
                        }

                        writeln!(f, ">;")?;
                    }
                },
            },
        }
    }

    Ok(any_props)
}
