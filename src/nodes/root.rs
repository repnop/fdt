use super::{
    aliases::Aliases,
    chosen::Chosen,
    cpus::Cpus,
    memory::{Memory, ReservedMemory},
    FallibleNode, FallibleRoot, IntoSearchableNodeName, Node, RawNode, SearchableNodeName,
};
use crate::{
    parsing::{aligned::AlignedParser, BigEndianToken, NoPanic, Panic, ParseError, Parser, ParserWithMode},
    properties::{cells::CellSizes, Compatible, PHandle, Property},
    FdtError,
};

/// [Devicetree 3.2. Root
/// node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#root-node)
///
/// The devicetree has a single root node of which all other device nodes are
/// descendants. The full path to the root node is `/`.
pub struct Root<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    pub(crate) node: FallibleNode<'a, P>,
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
            self.node.property::<CellSizes>()?.ok_or(FdtError::MissingRequiredProperty("#address-cells/#size-cells"))
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
    #[track_caller]
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
    #[track_caller]
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
    #[track_caller]
    pub fn chassis_type(&self) -> P::Output<Option<&'a str>> {
        P::to_output(crate::tryblock!({
            match self.node.fallible().properties()?.find("serial-number")? {
                Some(prop) => Ok(Some(prop.as_value()?)),
                None => Ok(None),
            }
        }))
    }

    /// [Devicetree 3.3. `/aliases`
    /// node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#aliases-node)
    ///
    /// **Required**s
    ///
    /// A devicetree may have an aliases node (`/aliases`) that defines one or
    /// more alias properties. The alias node shall be at the root of the
    /// devicetree and have the node name `/aliases`.
    ///
    /// Each property of the `/aliases` node defines an alias. The property name
    /// specifies the alias name. The property value specifies the full path to
    /// a node in the devicetree. For example, the property `serial0 =
    /// "/simple-bus@fe000000/serial@llc500"` defines the alias `serial0`.
    #[track_caller]
    pub fn aliases(&self) -> P::Output<Option<Aliases<'a, P>>> {
        P::to_output(crate::tryblock!({
            let this: FallibleRoot<'a, P> = Root { node: self.node };
            match this.find_node("/aliases")? {
                Some(node) => Ok(Some(Aliases { node })),
                None => Ok(None),
            }
        }))
    }

    /// [Devicetree 3.6. `/chosen`
    /// Node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#chosen-node)
    ///
    /// **Required**
    ///
    /// The `/chosen` node does not represent a real device in the system but
    /// describes parameters chosen or specified by the system firmware at run
    /// time. It shall be a child of the root node.
    #[track_caller]
    pub fn chosen(&self) -> P::Output<Chosen<'a, P>> {
        P::to_output(crate::tryblock!({
            let this: FallibleRoot<'a, P> = Root { node: self.node };
            match this.find_node("/chosen")? {
                Some(node) => Ok(Chosen { node }),
                None => Err(FdtError::MissingRequiredNode("/chosen")),
            }
        }))
    }

    /// [Devicetree 3.7.
    /// `/cpus`](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#cpus-node-properties)
    ///
    /// **Required**
    ///
    /// A `/cpus` node is required for all devicetrees. It does not represent a
    /// real device in the system, but acts as a container for child cpu nodes
    /// which represent the systems CPUs.
    #[track_caller]
    pub fn cpus(&self) -> P::Output<Cpus<'a, P>> {
        P::to_output(crate::tryblock!({
            let this: FallibleRoot<'a, P> = Root { node: self.node };
            match this.find_node("/cpus")? {
                Some(node) => Ok(Cpus { node }),
                None => Err(FdtError::MissingRequiredNode("/cpus")),
            }
        }))
    }

    /// [Devicetree 3.4. `/memory`
    /// node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#memory-node)
    ///
    /// **Required**
    ///
    /// A memory device node is required for all devicetrees and describes the
    /// physical memory layout for the system. If a system has multiple ranges
    /// of memory, multiple memory nodes can be created, or the ranges can be
    /// specified in the `reg` property of a single memory node.
    #[track_caller]
    pub fn memory(&self) -> P::Output<Memory<'a, P>> {
        P::to_output(crate::tryblock!({
            let this: FallibleRoot<'a, P> = Root { node: self.node };
            match this.find_node("/memory")? {
                Some(node) => Ok(Memory { node }),
                None => Err(FdtError::MissingRequiredNode("/memory")),
            }
        }))
    }

    /// [Devicetree 3.5. `/reserved-memory`
    /// node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#reserved-memory-node)
    ///
    /// Reserved memory is specified as a node under the `/reserved-memory`
    /// node. The operating system shall exclude reserved memory from normal
    /// usage. One can create child nodes describing particular reserved
    /// (excluded from normal use) memory regions. Such memory regions are
    /// usually designed for the special usage by various device drivers.
    #[track_caller]
    pub fn reserved_memory(&self) -> P::Output<ReservedMemory<'a, P>> {
        P::to_output(crate::tryblock!({
            let this: FallibleRoot<'a, P> = Root { node: self.node };
            match this.find_node("/reserved-memory")? {
                Some(node) => Ok(ReservedMemory { node }),
                None => Err(FdtError::MissingRequiredNode("/reserved-memory")),
            }
        }))
    }

    /// Attempt to resolve a [`PHandle`] to the node containing a `phandle`
    /// property with the value
    #[track_caller]
    pub fn resolve_phandle(&self, phandle: PHandle) -> P::Output<Option<Node<'a, P>>> {
        P::to_output(crate::tryblock!({
            let this: FallibleRoot<'a, P> = Root { node: self.node.fallible() };
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
    #[track_caller]
    pub fn find_all_nodes_with_name<'b>(self, name: &'b str) -> P::Output<AllNodesWithNameIter<'a, 'b, P>> {
        P::to_output(crate::tryblock!({
            let this: FallibleRoot<'a, P> = Root { node: self.node };
            Ok(AllNodesWithNameIter { iter: this.all_nodes()?, name })
        }))
    }

    /// Attempt to find a node with the given name, returning the first node
    /// with a name that matches `name` in depth-first order
    #[track_caller]
    pub fn find_node_by_name(self, name: &str) -> P::Output<Option<Node<'a, P>>> {
        P::to_output(crate::tryblock!({
            let this: FallibleRoot<'a, P> = Root { node: self.node };
            this.find_all_nodes_with_name(name)?.next().transpose().map(|n| n.map(|n| n.alt()))
        }))
    }

    /// Attempt to find a node with the given path (with an optional unit
    /// address, defaulting to the first matching name if omitted). If you only
    /// have the node name but not the path, use [`Root::find_node_by_name`] instead.
    #[track_caller]
    pub fn find_node(self, path: &str) -> P::Output<Option<Node<'a, P>>> {
        if path == "/" {
            return P::to_output(Ok(Some(self.node.alt())));
        }

        let fallible_self: FallibleRoot<'a, P> = Root { node: self.node.fallible() };

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
            let this: FallibleRoot<'a, P> = Root { node: self.node };
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
        let res = crate::tryblock!({
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
    pub(crate) iter: AllNodesIter<'a, (P::Parser, NoPanic)>,
    pub(crate) name: &'b str,
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
    #[allow(clippy::type_complexity)]
    pub(crate) iter: core::iter::FilterMap<
        AllNodesIter<'a, (P::Parser, NoPanic)>,
        fn(
            Result<(usize, FallibleNode<'a, P>), FdtError>,
        ) -> Option<Result<(FallibleNode<'a, P>, Compatible<'a>), FdtError>>,
    >,
    pub(crate) with: &'b [&'b str],
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
    pub(crate) parser: P,
    pub(crate) parents: [&'a [<P as Parser<'a>>::Granularity]; 16],
    pub(crate) parent_index: usize,
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

        let res = crate::tryblock!({
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
