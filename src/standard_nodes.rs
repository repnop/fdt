// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    cell_collector::{BuildCellCollector, CellCollector, CollectCellsError},
    nodes::{FallibleNode, IntoSearchableNodeName, Node, RawNode, SearchableNodeName},
    parsing::{aligned::AlignedParser, BigEndianToken, NoPanic, Panic, ParseError, Parser, ParserWithMode},
    properties::{
        cells::{AddressCells, CellSizes},
        values::StringList,
        Compatible, PHandle, Property,
    },
    tryblock, FdtError,
};

/// Represents the `/chosen` node with specific helper methods
pub struct Chosen<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    pub(crate) node: Node<'a, P>,
}

impl<'a, P: ParserWithMode<'a>> Chosen<'a, P> {
    /// Contains the bootargs, if they exist
    #[track_caller]
    pub fn bootargs(self) -> P::Output<Option<&'a str>> {
        P::to_output(crate::tryblock!({
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
        }))
    }

    /// Looks up the `stdout-path` property and returns the [`StdInOutPath`]
    /// representing the path. The path may be an alias and require being
    /// resolved with [`Alias::resolve`] before being used in conjunction with
    /// [`Root::find_node`]. For more information about the path parameters, see
    /// [`StdInOutPath::params`].
    #[track_caller]
    pub fn stdout(self) -> P::Output<Option<StdInOutPath<'a>>> {
        P::to_output(crate::tryblock!({
            let node = self.node.fallible();
            node.properties()?
                .into_iter()
                .find_map(|n| match n {
                    Err(e) => Some(Err(e)),
                    Ok(property) => match property.name() == "stdout-path" {
                        false => None,
                        true => Some(property.as_value::<&'a str>().map_err(Into::into).map(|s| {
                            let (path, params) =
                                s.split_once(':').map_or_else(|| (s, None), |(name, params)| (name, Some(params)));
                            StdInOutPath { path, params }
                        })),
                    },
                })
                .transpose()
        }))
    }

    /// Looks up the `stdin-path` property and returns the [`StdInOutPath`]
    /// representing the path. The path may be an alias and require being
    /// resolved with [`Alias::resolve`] before being used in conjunction with
    /// [`Root::find_node`]. For more information about the path parameters, see
    /// [`StdInOutPath::params`].
    #[track_caller]
    pub fn stdin(self) -> P::Output<Option<StdInOutPath<'a>>> {
        P::to_output(crate::tryblock!({
            let node = self.node.fallible();
            node.properties()?
                .into_iter()
                .find_map(|n| match n {
                    Err(e) => Some(Err(e)),
                    Ok(property) => match property.name() == "stdin-path" {
                        false => None,
                        true => Some(property.as_value::<&str>().map_err(Into::into).map(|s| {
                            let (path, params) =
                                s.split_once(':').map_or_else(|| (s, None), |(name, params)| (name, Some(params)));
                            StdInOutPath { path, params }
                        })),
                    },
                })
                .transpose()
        }))
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
    /// assert_eq!((stdout.path(), stdout.params()), ("/soc/uart@10000000", None));
    /// assert_eq!((stdin.path(), stdin.params()), ("/soc/uart@10000000", Some("115200")));
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
    /// [Devicetree 3.2. Root
    /// node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#root-node)
    ///
    /// **Required**
    ///
    /// Specifies the number of <u32> cells to represent the address and length
    /// in the reg property in children of root.
    #[track_caller]
    pub fn cell_sizes(self) -> P::Output<CellSizes> {
        P::to_output(crate::tryblock!({
            self.node
                .fallible()
                .property::<CellSizes>()?
                .ok_or(FdtError::MissingRequiredProperty("#address-cells/#size-cells"))
        }))
    }

    /// [Devicetree 3.2. Root
    /// node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#root-node)
    ///
    /// **Required**
    ///
    /// Specifies a string that uniquely identifies the model of the system
    /// board. The recommended format is "manufacturer,model-number".
    #[track_caller]
    pub fn model(self) -> P::Output<&'a str> {
        P::to_output(crate::tryblock!({
            let node = self.node.fallible();
            node.properties()?.find("model").and_then(|p| {
                p.ok_or(FdtError::MissingRequiredProperty("model"))?.as_value::<&'a str>().map_err(Into::into)
            })
        }))
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
    pub fn compatible(&self) -> P::Output<Compatible<'a>> {
        P::to_output(crate::tryblock!({
            <Compatible as Property<'a, P>>::parse(self.node.fallible(), self.node.make_root()?)?
                .ok_or(FdtError::MissingRequiredProperty("compatible"))
        }))
    }

    /// [Devicetree 3.2. Root
    /// node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#root-node)
    ///
    /// **Optional**
    ///
    /// Specifies a string representing the device’s serial number.
    pub fn serial_number(&self) -> P::Output<Option<&'a str>> {
        P::to_output(crate::tryblock!({
            match self.node.fallible().properties()?.find("serial-number")? {
                Some(prop) => Ok(Some(prop.as_value()?)),
                None => Ok(None),
            }
        }))
    }

    /// [Devicetree 3.2. Root
    /// node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#root-node)
    ///
    /// **Optional, but Recommended**
    ///
    /// Specifies a string that identifies the form-factor of the system. The
    /// property value can be one of:
    ///
    /// * "desktop"
    /// * "laptop"
    /// * "convertible"
    /// * "server"
    /// * "tablet"
    /// * "handset"
    /// * "watch"
    /// * "embedded"
    pub fn chassis_type(&self) -> P::Output<Option<&'a str>> {
        P::to_output(crate::tryblock!({
            match self.node.fallible().properties()?.find("serial-number")? {
                Some(prop) => Ok(Some(prop.as_value()?)),
                None => Ok(None),
            }
        }))
    }

    /// Attempt to resolve a [`PHandle`] to the node containing a `phandle`
    /// property with the value
    #[track_caller]
    pub fn resolve_phandle(&self, phandle: PHandle) -> P::Output<Option<Node<'a, P>>> {
        P::to_output(crate::tryblock!({
            let this = Root { node: self.node.fallible() };
            for node in this.all_nodes()? {
                let (_, node) = node?;
                if node.property::<PHandle>()? == Some(phandle) {
                    return Ok(Some(node.alt()));
                }
            }

            Ok(None)
        }))
    }

    /// Returns an iterator that yields every node with the name that matches
    /// `name` in depth-first order
    pub fn find_all_nodes_with_name<'b>(self, name: &'b str) -> P::Output<AllNodesWithNameIter<'a, 'b, P>> {
        P::to_output(crate::tryblock!({
            let this = Root { node: self.node.fallible() };
            Ok(AllNodesWithNameIter { iter: this.all_nodes()?, name })
        }))
    }

    /// Attempt to find a node with the given name, returning the first node
    /// with a name that matches `name` in depth-first order
    pub fn find_node_by_name(self, name: &str) -> P::Output<Option<Node<'a, P>>> {
        P::to_output(crate::tryblock!({
            let this = Root { node: self.node.fallible() };
            this.find_all_nodes_with_name(name)?.next().transpose().map(|n| n.map(|n| n.alt()))
        }))
    }

    /// Attempt to find a node with the given path (with an optional unit
    /// address, defaulting to the first matching name if omitted). If you only
    /// have the node name but not the path, use [`Root::find_node_by_name`] instead.
    #[track_caller]
    pub fn find_node(self, path: &str) -> P::Output<Option<Node<'a, P>>> {
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

    /// Returns an iterator over every node within the devicetree which is
    /// compatible with at least one of the compatible strings contained within
    /// `with`
    #[track_caller]
    pub fn all_compatible<'b>(self, with: &'b [&str]) -> P::Output<AllCompatibleIter<'a, 'b, P>> {
        P::to_output(crate::tryblock!({
            let this = Root { node: self.node.fallible() };
            let f: fn(_) -> _ = |node: Result<(usize, FallibleNode<'a, P>), FdtError>| match node
                .and_then(|(_, n)| Ok((n, n.property::<Compatible>()?)))
            {
                Ok((n, compatible)) => Some(Ok((n, compatible?))),
                Err(e) => Some(Err(e)),
            };

            let iter = this.all_nodes()?.filter_map(f);

            Ok(AllCompatibleIter { iter, with })
        }))
    }

    /// Returns an iterator over each node in the tree, depth-first, along with
    /// its depth in the tree
    #[track_caller]
    pub fn all_nodes(self) -> P::Output<AllNodesIter<'a, P>> {
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

        P::to_output(Ok(AllNodesIter {
            parser,
            parents: [
                self.node.this.as_slice(),
                &[],
                &[],
                &[],
                &[],
                &[],
                &[],
                &[],
                &[],
                &[],
                &[],
                &[],
                &[],
                &[],
                &[],
                &[],
            ],
            parent_index: 0,
        }))
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

pub struct AllNodesWithNameIter<'a, 'b, P: ParserWithMode<'a>> {
    iter: AllNodesIter<'a, (P::Parser, NoPanic)>,
    name: &'b str,
}

impl<'a, 'b, P: ParserWithMode<'a>> Iterator for AllNodesWithNameIter<'a, 'b, P> {
    type Item = P::Output<Node<'a, P>>;

    #[track_caller]
    fn next(&mut self) -> Option<Self::Item> {
        for next in self.iter.by_ref() {
            match next.and_then(|(_, n)| Ok((n, n.name()?))) {
                Ok((node, name)) => match name.name == self.name {
                    true => return Some(P::to_output(Ok(node.alt()))),
                    false => continue,
                },
                Err(e) => return Some(P::to_output(Err(e))),
            }
        }

        None
    }
}

/// See [`Root::all_compatible`]
pub struct AllCompatibleIter<'a, 'b, P: ParserWithMode<'a>> {
    iter: core::iter::FilterMap<
        AllNodesIter<'a, (P::Parser, NoPanic)>,
        fn(
            Result<(usize, FallibleNode<'a, P>), FdtError>,
        ) -> Option<Result<(FallibleNode<'a, P>, Compatible<'a>), FdtError>>,
    >,
    with: &'b [&'b str],
}

impl<'a, 'b, P: ParserWithMode<'a>> Iterator for AllCompatibleIter<'a, 'b, P> {
    type Item = P::Output<Node<'a, P>>;

    #[track_caller]
    fn next(&mut self) -> Option<Self::Item> {
        for next in self.iter.by_ref() {
            match next {
                Ok((node, compatible)) => match self.with.iter().copied().any(|c| compatible.compatible_with(c)) {
                    true => return Some(P::to_output(Ok(node.alt()))),
                    false => continue,
                },
                Err(e) => return Some(P::to_output(Err(e))),
            }
        }

        None
    }
}

pub struct AllNodesIter<'a, P: ParserWithMode<'a>> {
    parser: P,
    parents: [&'a [<P as Parser<'a>>::Granularity]; 16],
    parent_index: usize,
}

impl<'a, P: ParserWithMode<'a>> Iterator for AllNodesIter<'a, P> {
    type Item = P::Output<(usize, Node<'a, P>)>;

    #[track_caller]
    fn next(&mut self) -> Option<Self::Item> {
        while let Ok(BigEndianToken::END_NODE) = self.parser.peek_token() {
            let _ = self.parser.advance_token();
            self.parent_index = self.parent_index.saturating_sub(1);
        }

        match self.parser.advance_token() {
            Ok(BigEndianToken::BEGIN_NODE) => self.parent_index += 1,
            Ok(BigEndianToken::END) | Err(FdtError::ParseError(ParseError::UnexpectedEndOfData)) => return None,
            Ok(_) => return Some(P::to_output(Err(FdtError::ParseError(ParseError::UnexpectedToken)))),
            Err(e) => return Some(P::to_output(Err(e))),
        }

        let starting_data = self.parser.data();

        match self.parents.get_mut(self.parent_index) {
            Some(idx) => *idx = starting_data,
            // FIXME: what makes sense for this to return?
            None => return None,
        }

        let node = Some(P::to_output(Ok((
            self.parent_index,
            Node {
                this: RawNode::new(starting_data),
                parent: self.parents.get(self.parent_index.saturating_sub(1)).map(|parent| RawNode::new(parent)),
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

/// [Devicetree 3.3. `/aliases`
/// node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#aliases-node)
///
/// A devicetree may have an aliases node (`/aliases`) that defines one or more
/// alias properties. The alias node shall be at the root of the devicetree and
/// have the node name `/aliases`.
///
/// Each property of the `/aliases` node defines an alias. The property name
/// specifies the alias name. The property value specifies the full path to a
/// node in the devicetree. For example, the property `serial0 =
/// "/simple-bus@fe000000/serial@llc500"` defines the alias `serial0`.
///
/// An alias value is a device path and is encoded as a string. The value
/// represents the full path to a node, but the path does not need to refer to a
/// leaf node.
///
/// A client program may use an alias property name to refer to a full device
/// path as all or part of its string value. A client program, when considering
/// a string as a device path, shall detect and use the alias.
///
/// ### Example
///
/// ```norust
/// aliases {
///     serial0 = "/simple-bus@fe000000/serial@llc500";
///     ethernet0 = "/simple-bus@fe000000/ethernet@31c000";
/// };
/// ```
///
/// Given the alias `serial0`, a client program can look at the `/aliases` node
/// and determine the alias refers to the device path
/// `/simple-bus@fe000000/serial@llc500`.
#[derive(Debug, Clone, Copy)]
pub struct Aliases<'a, P: ParserWithMode<'a>> {
    pub(crate) node: FallibleNode<'a, P>,
}

impl<'a, P: ParserWithMode<'a>> Aliases<'a, P> {
    /// Attempt to resolve an alias to a node name.
    pub fn resolve_name(self, alias: &str) -> P::Output<Option<&'a str>> {
        P::to_output(crate::tryblock!({
            self.node.properties()?.find(alias)?.map(|p| p.as_value().map_err(Into::into)).transpose()
        }))
    }

    /// Attempt resolve an alias to the aliased-to node.
    pub fn resolve(self, alias: &str) -> P::Output<Option<Node<'a, P>>> {
        P::to_output(crate::tryblock!({
            let Some(path) = Aliases::<(_, NoPanic)> { node: self.node }.resolve_name(alias)? else {
                return Ok(None);
            };

            self.node.make_root::<P::Parser>()?.find_node(path).map(|r| r.map(|n| n.alt()))
        }))
    }
}

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
    pub fn cell_sizes(&self) -> P::Output<CellSizes> {
        P::to_output(
            self.node.property().and_then(|p| p.ok_or(FdtError::MissingRequiredProperty("#address-cells/#size-cells"))),
        )
    }

    /// Attempt to find a common `timebase-frequency` property inside of this
    /// node, which will only exist if there is a common value between the child
    /// `cpu` nodes. See [`Cpu::timebase_frequency`] for documentation about the
    /// `timebase-frequency` property.
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
    /// [Devicetree 3.8.1
    /// `reg`](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#general-properties-of-cpus-cpu-nodes)
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

    /// [Devicetree 3.8.1
    /// `clock-frequency`](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#general-properties-of-cpus-cpu-nodes)
    ///
    /// **Required**
    ///
    /// Specifies the current clock speed of the CPU in Hertz. The value is a
    /// `<prop-encoded-array>` in one of two forms:
    ///
    /// * A 32-bit integer consisting of one `<u32>` specifying the frequency.
    /// * A 64-bit integer represented as a `<u64>` specifying the frequency.
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

    /// [Devicetree 3.8.1
    /// `timebase-frequency`](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#general-properties-of-cpus-cpu-nodes)
    ///
    /// **Required**
    ///
    /// Specifies the current frequency at which the timebase and decrementer
    /// registers are updated (in Hertz). The value is a `<prop-encoded-array>` in
    /// one of two forms:
    ///
    /// * A 32-bit integer consisting of one `<u32>` specifying the frequency.
    /// * A 64-bit integer represented as a `<u64>`.
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

    /// [Devicetree 3.8.1
    /// `status`](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#general-properties-of-cpus-cpu-nodes)
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
    pub fn status(&self) -> P::Output<Option<CpuStatus>> {
        P::to_output(crate::tryblock!({
            let Some(status) = self.node.properties()?.find("status")? else {
                return Ok(None);
            };

            Ok(Some(CpuStatus(status.as_value()?)))
        }))
    }

    /// [Devicetree 3.8.1
    /// `enable-method`](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#general-properties-of-cpus-cpu-nodes)
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

#[derive(Debug)]
pub struct CpuIdsIter<'a, C: CellCollector> {
    reg: &'a [u8],
    address_cells: usize,
    _collector: core::marker::PhantomData<*mut C>,
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
