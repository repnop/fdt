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
