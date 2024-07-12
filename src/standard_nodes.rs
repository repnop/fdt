// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    nodes::{IntoSearchableNodeName, Node, RawNode, SearchableNodeName},
    parsing::{aligned::AlignedParser, BigEndianToken, Panic, ParseError, Parser, ParserWithMode},
    properties::{CellSizes, Compatible, PHandle, Property},
    tryblock, FdtError,
};

/// Represents the `/chosen` node with specific helper methods
pub struct Chosen<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    pub(crate) node: Node<'a, P>,
}

impl<'a, P: ParserWithMode<'a> + 'a> Chosen<'a, P> {
    /// Contains the bootargs, if they exist
    #[track_caller]
    pub fn bootargs(self) -> P::Output<Option<&'a str>> {
        P::to_output(crate::tryblock! {
            let node = self.node.fallible();
            for prop in node.properties()?.into_iter().flatten() {
                if prop.name() == "bootargs" {
                    return Ok(Some(
                        core::str::from_utf8(&prop.value()[..prop.value().len() - 1])
                            .map_err(|_| FdtError::ParseError(ParseError::InvalidCStrValue))?,
                    ));
                }
            }

            Ok(None)
        })
    }

    /// Looks up the `stdout-path` property and returns the [`StdInOutPath`]
    /// representing the path. The path may be an alias and require being
    /// resolved with [`Alias::resolve`] before being used in conjunction with
    /// [`Root::find_node`]. For more information about the path parameters, see
    /// [`StdInOutPath::params`].
    #[track_caller]
    pub fn stdout(self) -> P::Output<Option<StdInOutPath<'a>>> {
        P::to_output(crate::tryblock! {
            let node = self.node.fallible();
            node.properties()?.into_iter().find_map(|n| match n {
                Err(e) => Some(Err(e)),
                Ok(property) => match property.name() == "stdout-path" {
                    false => None,
                    true => Some(
                        property
                            .to::<&'a str>()
                            .map_err(Into::into)
                            .map(|s| {
                                let (path, params) = Self::split_stdinout_property(s);
                                StdInOutPath { path, params }
                            })
                    ),
                },
            }).transpose()
        })
    }

    /// Looks up the `stdin-path` property and returns the [`StdInOutPath`]
    /// representing the path. The path may be an alias and require being
    /// resolved with [`Alias::resolve`] before being used in conjunction with
    /// [`Root::find_node`]. For more information about the path parameters, see
    /// [`StdInOutPath::params`].
    #[track_caller]
    pub fn stdin(self) -> P::Output<Option<StdInOutPath<'a>>> {
        P::to_output(crate::tryblock! {
            let node = self.node.fallible();
            node.properties()?.into_iter().find_map(|n| match n {
                Err(e) => Some(Err(e)),
                Ok(property) => match property.name() == "stdin-path" {
                    false => None,
                    true => Some(
                        property
                            .to::<&'a str>()
                            .map_err(Into::into)
                            .map(|s| {
                                let (path, params) = Self::split_stdinout_property(s);
                                StdInOutPath { path, params }
                            })
                    ),
                },
            }).transpose()
        })
    }

    fn split_stdinout_property(property: &str) -> (&str, Option<&str>) {
        property
            .split_once(':')
            .map_or_else(|| (property, None), |(name, params)| (name, Some(params)))
    }
}

impl<'a, P: ParserWithMode<'a>> Clone for Chosen<'a, P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, P: ParserWithMode<'a>> Copy for Chosen<'a, P> {}

pub struct StdInOutPath<'a> {
    path: &'a str,
    params: Option<&'a str>,
}

impl<'a> StdInOutPath<'a> {
    /// Path to the node representing the stdin/stdout device. This node path
    /// may be an alias, which can be resolved with [`Aliases::resolve`]. To be
    /// used in conjunction with [`Root::find_node`].
    pub fn path(&self) -> &'a str {
        self.path
    }

    /// Optional parameters specified by the stdin/stdout property value. See
    /// https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#chosen-node
    ///
    /// Example:
    ///
    /// ```dts
    /// / {
    ///     chosen {
    ///         stdout-path = "/soc/uart@10000000:115200";
    ///         stdin-path = "/soc/uart@10000000";
    ///     }
    /// }
    /// ```
    ///
    /// ```rust
    /// # let fdt = fdt::Fdt::new_unaligned(include_bytes!("../dtb/test.dtb")).unwrap();
    /// # let chosen = fdt.root().chosen();
    /// let stdout = chosen.stdout().unwrap();
    /// let stdin = chosen.stdin().unwrap();
    ///
    /// assert_eq!((stdout.path(), stdout.params()), ("soc/uart@10000000", None));
    /// assert_eq!((stdin.path(), stdin.params()), ("soc/uart@10000000", Some("115200")));
    /// ```
    pub fn params(&self) -> Option<&'a str> {
        self.params
    }
}

/// Represents the root (`/`) node with specific helper methods
pub struct Root<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    pub(crate) node: Node<'a, P>,
}

impl<'a, P: ParserWithMode<'a>> Root<'a, P> {
    /// Root node cell sizes
    #[track_caller]
    pub fn cell_sizes(self) -> P::Output<CellSizes> {
        P::to_output(crate::tryblock! {
            self.node.fallible().property::<CellSizes>()?.ok_or(FdtError::MissingRequiredProperty("#address-cells/#size-cells"))
        })
    }

    /// [Devicetree 3.2. Root
    /// node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#root-node)
    ///
    /// Specifies a string that uniquely identifies the model of the system
    /// board. The recommended format is "manufacturer,model-number".
    #[track_caller]
    pub fn model(self) -> P::Output<&'a str> {
        P::to_output(crate::tryblock! {
            let node = self.node.fallible();
            node
                .properties()?
                .into_iter()
                .find_map(|n| match n {
                    Err(e) => Some(Err(e)),
                    Ok(property) => match property.name() == "model" {
                        false => None,
                        true => Some(
                            property
                                .to::<&'a str>()
                                .map_err(Into::into)
                        ),
                    },
                })
                .transpose()?
                .ok_or(FdtError::MissingRequiredProperty("model"))
        })
    }

    /// [Devicetree 3.2. Root
    /// node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#root-node)
    ///
    /// **Required**
    ///
    /// Specifies a list of platform architectures with which this platform is
    /// compatible. This property can be used by operating systems in selecting
    /// platform specific code. The recommended form of the property value is:
    /// `"manufacturer,model"`
    ///
    /// For example: `compatible = "fsl,mpc8572ds"`
    pub fn compatible(self) -> P::Output<Compatible<'a>> {
        P::to_output(crate::tryblock! {
                <Compatible as Property<'a, P>>::parse(self.node.fallible(), self.node.make_root()?)?
                    .ok_or(FdtError::MissingRequiredProperty("compatible"))
        })
    }

    /// Returns an iterator over all of the available properties
    // pub fn properties(self) -> impl Iterator<Item = NodeProperty<'a>> + 'a {
    //     self.node.properties()
    // }

    /// Attempts to find the a property by its name
    // pub fn property(self, name: &str) -> Option<NodeProperty<'a>> {
    //     self.node.properties().find(|p| p.name == name)
    // }

    #[track_caller]
    pub fn resolve_phandle(self, phandle: PHandle) -> P::Output<Option<Node<'a, P>>> {
        P::to_output(crate::tryblock! {
            let this = Root { node: self.node.fallible() };
            for node in this.all_nodes()? {
                let (_, node) = node?;
                if node.property::<PHandle>()? == Some(phandle) {
                    return Ok(Some(node.alt()));
                }
            }

            Ok(None)
        })
    }

    #[track_caller]
    pub fn find_node(self, path: &str) -> P::Output<Option<Node<'a, P>>>
    where
        P: 'a,
    {
        if path == "/" {
            return P::to_output(Ok(Some(self.node)));
        }

        let fallible_self = Root { node: self.node.fallible() };

        let mut current_depth = 1;
        let mut all_nodes = match fallible_self.all_nodes() {
            Ok(iter) => iter,
            Err(e) => return P::to_output(Err(e)),
        };

        let mut found_node = None;
        'outer: for component in path.trim_start_matches('/').split('/') {
            let component_name = IntoSearchableNodeName::into_searchable_node_name(component);

            loop {
                let (depth, next_node) = match all_nodes.next() {
                    Some(Ok(next)) => next,
                    Some(Err(e)) => return P::to_output(Err(e)),
                    None => return P::to_output(Ok(None)),
                };

                if depth < current_depth {
                    return P::to_output(Ok(None));
                }

                let name = match next_node.name() {
                    Ok(name) => name,
                    Err(e) => return P::to_output(Err(e)),
                };

                let name_eq = match component_name {
                    SearchableNodeName::Base(cname) => cname == name.name,
                    SearchableNodeName::WithUnitAddress(cname) => cname == name,
                };

                if name_eq {
                    found_node = Some(next_node);
                    current_depth = depth;
                    continue 'outer;
                }
            }
        }

        P::to_output(Ok(found_node.map(|n| n.alt::<P>())))
    }

    pub fn all_nodes(self) -> P::Output<AllNodesIterator<'a, P>> {
        let mut parser = P::new(self.node.this.as_slice(), self.node.strings, self.node.structs);
        let res = tryblock!({
            parser.advance_cstr()?;

            while parser.peek_token()? == BigEndianToken::PROP {
                parser.parse_raw_property()?;
            }

            Ok(())
        });

        if let Err(e) = res {
            return P::to_output(Err(e));
        }

        P::to_output(Ok(AllNodesIterator { parser, parents: [&[]; 16], parent_index: 0 }))
    }
}

impl<'a, P: ParserWithMode<'a>> Copy for Root<'a, P> {}
impl<'a, P: ParserWithMode<'a>> Clone for Root<'a, P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, P: ParserWithMode<'a>> core::fmt::Debug for Root<'a, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Root").finish_non_exhaustive()
    }
}

pub struct AllNodesIterator<'a, P: ParserWithMode<'a>> {
    parser: P,
    parents: [&'a [<P as Parser<'a>>::Granularity]; 16],
    parent_index: usize,
}

impl<'a, P: ParserWithMode<'a>> Iterator for AllNodesIterator<'a, P> {
    type Item = P::Output<(usize, Node<'a, P>)>;

    #[track_caller]
    fn next(&mut self) -> Option<Self::Item> {
        while let Ok(BigEndianToken::END_NODE) = self.parser.peek_token() {
            let _ = self.parser.advance_token();
            self.parent_index = self.parent_index.saturating_sub(1);
        }

        match self.parser.advance_token() {
            Ok(BigEndianToken::BEGIN_NODE) => self.parent_index += 1,
            Ok(BigEndianToken::END)
            | Err(FdtError::ParseError(ParseError::UnexpectedEndOfData)) => return None,
            Ok(_) => {
                return Some(P::to_output(Err(FdtError::ParseError(ParseError::UnexpectedToken))))
            }
            Err(e) => return Some(P::to_output(Err(e))),
        }

        let starting_data = self.parser.data();

        match self.parents.get_mut(self.parent_index.saturating_sub(1)) {
            Some(idx) => *idx = starting_data,
            // FIXME: what makes sense for this to return?
            None => return None,
        }

        let node = Some(P::to_output(Ok((
            self.parent_index,
            Node {
                this: RawNode::new(starting_data),
                parent: self.parents.get(self.parent_index).map(|parent| RawNode::new(parent)),
                strings: self.parser.strings(),
                structs: self.parser.structs(),
                _mode: core::marker::PhantomData,
            },
        ))));

        let res = tryblock!({
            self.parser.advance_cstr()?;

            while self.parser.peek_token()? == BigEndianToken::PROP {
                self.parser.parse_raw_property()?;
            }

            Ok(())
        });

        if let Err(e) = res {
            return Some(P::to_output(Err(e)));
        }

        node
    }
}

// /// Represents the `/aliases` node with specific helper methods
// #[derive(Debug, Clone, Copy)]
// pub struct Aliases<'b, 'a: 'b> {
//     pub(crate) header: &'b Fdt<'a, crate::UnalignedParser<'a>>,
//     pub(crate) node: FdtNode<'b, 'a>,
// }

// impl<'b, 'a: 'b> Aliases<'b, 'a> {
//     /// Attempt to resolve an alias to a node name
//     pub fn resolve(self, alias: &str) -> Option<&'a str> {
//         self.node
//             .properties()
//             .find(|p| p.name == alias)
//             .and_then(|p| core::str::from_utf8(p.value).map(|s| s.trim_end_matches('\0')).ok())
//     }

//     /// Attempt to find the node specified by the given alias
//     pub fn resolve_node(self, alias: &str) -> Option<FdtNode<'b, 'a>> {
//         self.resolve(alias).and_then(|name| self.header.find_node(name))
//     }

//     /// Returns an iterator over all of the available aliases
//     pub fn all(self) -> impl Iterator<Item = (&'a str, &'a str)> + 'b {
//         self.node.properties().filter_map(|p| {
//             Some((p.name, core::str::from_utf8(p.value).map(|s| s.trim_end_matches('\0')).ok()?))
//         })
//     }
// }

// /// Represents a `/cpus/cpu*` node with specific helper methods
// #[derive(Debug, Clone, Copy)]
// pub struct Cpu<'b, 'a: 'b> {
//     pub(crate) parent: FdtNode<'b, 'a>,
//     pub(crate) node: FdtNode<'b, 'a>,
// }

// impl<'b, 'a: 'b> Cpu<'b, 'a> {
//     /// Return the IDs for the given CPU
//     pub fn ids(self) -> CpuIds<'a> {
//         let address_cells = self.node.parent_cell_sizes().address_cells;

//         CpuIds {
//             reg: self
//                 .node
//                 .properties()
//                 .find(|p| p.name == "reg")
//                 .expect("reg is a required property of cpu nodes"),
//             address_cells,
//         }
//     }

//     /// `clock-frequency` property
//     pub fn clock_frequency(self) -> usize {
//         self.node
//             .properties()
//             .find(|p| p.name == "clock-frequency")
//             .or_else(|| self.parent.property("clock-frequency"))
//             .map(|p| match p.value.len() {
//                 4 => BigEndianU32::from_bytes(p.value).unwrap().to_ne() as usize,
//                 8 => BigEndianU64::from_bytes(p.value).unwrap().to_ne() as usize,
//                 _ => unreachable!(),
//             })
//             .expect("clock-frequency is a required property of cpu nodes")
//     }

//     /// `timebase-frequency` property
//     pub fn timebase_frequency(self) -> usize {
//         self.node
//             .properties()
//             .find(|p| p.name == "timebase-frequency")
//             .or_else(|| self.parent.property("timebase-frequency"))
//             .map(|p| match p.value.len() {
//                 4 => BigEndianU32::from_bytes(p.value).unwrap().to_ne() as usize,
//                 8 => BigEndianU64::from_bytes(p.value).unwrap().to_ne() as usize,
//                 _ => unreachable!(),
//             })
//             .expect("timebase-frequency is a required property of cpu nodes")
//     }

//     /// Returns an iterator over all of the properties for the CPU node
//     pub fn properties(self) -> impl Iterator<Item = NodeProperty<'a>> + 'b {
//         self.node.properties()
//     }

//     /// Attempts to find the a property by its name
//     pub fn property(self, name: &str) -> Option<NodeProperty<'a>> {
//         self.node.properties().find(|p| p.name == name)
//     }
// }

// /// Represents the value of the `reg` property of a `/cpus/cpu*` node which may
// /// contain more than one CPU or thread ID
// #[derive(Debug, Clone, Copy)]
// pub struct CpuIds<'a> {
//     pub(crate) reg: NodeProperty<'a>,
//     pub(crate) address_cells: usize,
// }

// impl<'a> CpuIds<'a> {
//     /// The first listed CPU ID, which will always exist
//     pub fn first(self) -> usize {
//         match self.address_cells {
//             1 => BigEndianU32::from_bytes(self.reg.value).unwrap().to_ne() as usize,
//             2 => BigEndianU64::from_bytes(self.reg.value).unwrap().to_ne() as usize,
//             n => panic!("address-cells of size {} is currently not supported", n),
//         }
//     }

//     /// Returns an iterator over all of the listed CPU IDs
//     pub fn all(self) -> impl Iterator<Item = usize> + 'a {
//         let mut vals = FdtData::new(self.reg.value);
//         core::iter::from_fn(move || match vals.remaining() {
//             [] => None,
//             _ => Some(match self.address_cells {
//                 1 => vals.u32()?.to_ne() as usize,
//                 2 => vals.u64()?.to_ne() as usize,
//                 n => panic!("address-cells of size {} is currently not supported", n),
//             }),
//         })
//     }
// }

/// Represents the `/memory` node with specific helper methods
// #[derive(Debug, Clone, Copy)]
// pub struct Memory<'b, 'a: 'b> {
//     pub(crate) node: FdtNode<'b, 'a>,
// }

// impl<'a> Memory<'_, 'a> {
//     /// Returns an iterator over all of the available memory regions
//     pub fn regions(&self) -> impl Iterator<Item = MemoryRegion> + 'a {
//         self.node.reg().unwrap()
//     }

//     /// Returns the initial mapped area, if it exists
//     pub fn initial_mapped_area(&self) -> Option<MappedArea> {
//         let mut mapped_area = None;

//         if let Some(init_mapped_area) = self.node.property("initial_mapped_area") {
//             let mut stream = FdtData::new(init_mapped_area.value);
//             let effective_address = stream.u64().expect("effective address");
//             let physical_address = stream.u64().expect("physical address");
//             let size = stream.u32().expect("size");

//             mapped_area = Some(MappedArea {
//                 effective_address: effective_address.to_ne() as usize,
//                 physical_address: physical_address.to_ne() as usize,
//                 size: size.to_ne() as usize,
//             });
//         }

//         mapped_area
//     }
// }

/// An area described by the `initial-mapped-area` property of the `/memory`
/// node
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct MappedArea {
    /// Effective address of the mapped area
    pub effective_address: usize,
    /// Physical address of the mapped area
    pub physical_address: usize,
    /// Size of the mapped area
    pub size: usize,
}

/// A memory region
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MemoryRegion {
    /// Starting address represented as a pointer
    pub starting_address: *const u8,
    /// Size of the memory region
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
