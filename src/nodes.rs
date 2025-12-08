pub mod aliases;
pub mod chosen;
pub mod cpus;
pub mod memory;
pub mod root;

use crate::{
    helpers::FallibleNode,
    parsing::{
        aligned::AlignedParser, BigEndianToken, NoPanic, Panic, PanicMode, ParseError, Parser, ParserWithMode,
        StringsBlock, StructsBlock,
    },
    properties::{
        ranges::Ranges,
        reg::Reg,
        values::{InvalidPropertyValue, PropertyValue},
        Property,
    },
    FdtError,
};
use root::Root;

#[macro_export]
#[doc(hidden)]
macro_rules! tryblock {
    ($errty:ty, $block:block) => {{
        (|| -> Result<_, $errty> { $block })()
    }};
    ($block:block) => {{
        (|| -> Result<_, $crate::FdtError> { $block })()
    }};
}

/// Trait for extracting a [`Node`] from a wrapper type.
pub trait AsNode<'a, P: ParserWithMode<'a>> {
    #[allow(missing_docs)]
    fn as_node(&self) -> Node<'a, P>;
}

/// A node name that can searched with.
#[derive(Debug, Clone, Copy)]
pub enum SearchableNodeName<'a> {
    /// Node name without the unit address
    Base(&'a str),
    /// Node name with the unit address
    WithUnitAddress(NodeName<'a>),
}

/// Convert from a type that can potentially represent a node name that is able
/// to be searched for during lookup operations.
///
/// Currently, two type impls are defined on types other than
/// [`SearchableNodeName`]:
///   1. [`NodeName`]: corresponds directly to a
///          [`SearchableNodeName::WithUnitAddress`].
///   2. [`&str`]: attempts to parse the `str` as `name@unit-address`,
///          corresponding to [`SearchableNodeName::WithUnitAddress`], or as
///          just a base node name with no specified unit address, which will
///          resolve to the first node with that base name found.
pub trait IntoSearchableNodeName<'a>: Sized + crate::sealed::Sealed {
    #[allow(missing_docs)]
    fn into_searchable_node_name(self) -> SearchableNodeName<'a>;
}

impl crate::sealed::Sealed for SearchableNodeName<'_> {}
impl<'a> IntoSearchableNodeName<'a> for SearchableNodeName<'a> {
    fn into_searchable_node_name(self) -> SearchableNodeName<'a> {
        self
    }
}

impl crate::sealed::Sealed for NodeName<'_> {}
impl<'a> IntoSearchableNodeName<'a> for NodeName<'a> {
    fn into_searchable_node_name(self) -> SearchableNodeName<'a> {
        SearchableNodeName::WithUnitAddress(self)
    }
}

impl crate::sealed::Sealed for &'_ str {}
impl<'a> IntoSearchableNodeName<'a> for &'a str {
    fn into_searchable_node_name(self) -> SearchableNodeName<'a> {
        match self.rsplit_once('@') {
            Some((base, unit_address)) => {
                SearchableNodeName::WithUnitAddress(NodeName { name: base, unit_address: Some(unit_address) })
            }
            None => SearchableNodeName::Base(self),
        }
    }
}

/// A node name, split into its component parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeName<'a> {
    /// Node name.
    pub name: &'a str,
    /// Optional unit address specified after the `@`.
    pub unit_address: Option<&'a str>,
}

impl<'a> NodeName<'a> {
    /// Create a new [`NodeName`] from its raw parts.
    pub fn new(name: &'a str, unit_address: Option<&'a str>) -> Self {
        Self { name, unit_address }
    }
}

impl core::fmt::Display for NodeName<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.unit_address {
            Some(ua) => write!(f, "{}@{}", self.name, ua),
            None => write!(f, "{}", self.name),
        }
    }
}

/// A generic devicetree node.
pub struct Node<'a, P: ParserWithMode<'a>> {
    pub(crate) this: &'a RawNode<<P as Parser<'a>>::Granularity>,
    pub(crate) parent: Option<&'a RawNode<<P as Parser<'a>>::Granularity>>,
    pub(crate) strings: StringsBlock<'a>,
    pub(crate) structs: StructsBlock<'a, <P as Parser<'a>>::Granularity>,
    pub(crate) _mode: core::marker::PhantomData<*mut P>,
}

impl<'a, P: ParserWithMode<'a>> Node<'a, P> {
    /// Change the type of this node's [`PanicMode`] to [`NoPanic`].
    #[inline(always)]
    pub fn fallible(self) -> FallibleNode<'a, P> {
        self.alt()
    }

    /// Helper function for changing the [`PanicMode`] of this node.
    #[inline(always)]
    pub fn alt<P2: ParserWithMode<'a, Granularity = P::Granularity>>(self) -> Node<'a, P2> {
        Node {
            this: self.this,
            parent: self.parent,
            strings: self.strings,
            structs: self.structs,
            _mode: core::marker::PhantomData,
        }
    }

    pub(crate) fn make_root<P2: Parser<'a, Granularity = P::Granularity>>(
        self,
    ) -> Result<Root<'a, (P2, NoPanic)>, FdtError> {
        let mut parser = <(P2, NoPanic)>::new(self.structs.0, self.strings, self.structs);
        parser.parse_root().map(|node| Root { node })
    }

    /// The name of this node along with the optional unit address.
    #[inline]
    #[track_caller]
    pub fn name(&self) -> <P as PanicMode>::Output<NodeName<'a>> {
        P::to_output(
            P::new(&self.this.0, self.strings, self.structs)
                .advance_cstr()
                .and_then(|s| s.to_str().map_err(|_| FdtError::ParseError(ParseError::InvalidCStrValue)))
                .map(|s| {
                    if s.is_empty() {
                        return NodeName { name: "/", unit_address: None };
                    }

                    let (name, unit_address) = s.split_once('@').unzip();
                    NodeName { name: name.unwrap_or(s), unit_address }
                }),
        )
    }

    /// [Devicetree 3.8.1 General Properties of `/cpus/cpu*`
    /// nodes](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#general-properties-of-cpus-cpu-nodes)
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
    #[inline(always)]
    #[track_caller]
    pub fn reg(&self) -> P::Output<Option<Reg<'a>>> {
        self.property::<Reg<'a>>()
    }

    /// [Devicetree 2.3.8
    /// `ranges`](https://devicetree-specification.readthedocs.io/en/latest/chapter2-devicetree-basics.html#sect-standard-properties-ranges)
    ///
    /// Value type: `<empty>` or `<prop-encoded-array>` encoded as an arbitrary
    /// number of `(child-bus-address, parent-bus-address, length)` triplets.
    ///
    /// Description:
    ///
    /// The ranges property provides a means of defining a mapping or
    /// translation between the address space of the bus (the child address
    /// space) and the address space of the bus node’s parent (the parent
    /// address space).
    ///
    /// The format of the value of the ranges property is an arbitrary number of
    /// triplets of `(child-bus-address, parent-bus-address, length)`
    ///
    /// * The `child-bus-address` is a physical address within the child bus’
    ///   address space. The number of cells to represent the address is bus
    ///   dependent and can be determined from the `#address-cells` of this node
    ///   (the node in which the ranges property appears).
    /// * The `parent-bus-address` is a physical address within the parent bus’
    ///   address space. The number of cells to represent the parent address is
    ///   bus dependent and can be determined from the `#address-cells` property
    ///   of the node that defines the parent’s address space.
    /// * The `length` specifies the size of the range in the child’s address
    ///   space. The number of cells to represent the size can be determined
    ///   from the `#size-cells` of this node (the node in which the ranges
    ///   property appears).
    ///
    /// If the property is defined with an `<empty>` value, it specifies that
    /// the parent and child address space is identical, and no address
    /// translation is required.
    ///
    /// If the property is not present in a bus node, it is assumed that no
    /// mapping exists between children of the node and the parent address
    /// space.
    ///
    /// Address Translation Example:
    ///
    /// ```notrust
    /// soc {
    ///    compatible = "simple-bus";
    ///    #address-cells = <1>;
    ///    #size-cells = <1>;
    ///    ranges = <0x0 0xe0000000 0x00100000>;
    ///
    ///    serial@4600 {
    ///       device_type = "serial";
    ///       compatible = "ns16550";
    ///       reg = <0x4600 0x100>;
    ///       clock-frequency = <0>;
    ///       interrupts = <0xA 0x8>;
    ///       interrupt-parent = <&ipic>;
    ///    };
    /// };
    /// ```
    ///
    /// The soc node specifies a ranges property of
    ///
    /// ```notrust
    /// <0x0 0xe0000000 0x00100000>;
    /// ```
    ///
    /// This property value specifies that for a 1024 KB range of address space,
    /// a child node addressed at physical `0x0` maps to a parent address of
    /// physical `0xe0000000`. With this mapping, the serial device node can be
    /// addressed by a load or store at address `0xe0004600`, an offset of
    /// `0x4600` (specified in `reg`) plus the `0xe0000000` mapping specified in
    /// ranges.
    #[inline(always)]
    #[track_caller]
    pub fn ranges(&self) -> P::Output<Option<Ranges<'a>>> {
        self.property()
    }

    /// Returns [`NodeProperties`] which allows searching and iterating over
    /// this node's properties.
    #[inline]
    #[track_caller]
    pub fn properties(&self) -> P::Output<NodeProperties<'a, P>> {
        let mut parser = P::new(&self.this.0, self.strings, self.structs);
        let res = parser.advance_cstr();

        P::to_output(res.map(|_| NodeProperties {
            data: parser.data(),
            strings: self.strings,
            structs: self.structs,
            _mode: core::marker::PhantomData,
        }))
    }

    /// Attempt to find the property with the given name and extract the raw
    /// name and value.
    #[inline]
    #[track_caller]
    pub fn raw_property(&self, name: &str) -> P::Output<Option<NodeProperty<'a>>> {
        P::to_output(tryblock!({
            let this = self.fallible();
            this.properties()?.find(name)
        }))
    }

    /// Attempt to find and extract the specified property represented by
    /// `Prop`.
    #[track_caller]
    pub fn property<Prop: Property<'a, P>>(&self) -> P::Output<Option<Prop>> {
        P::to_output(crate::tryblock!({ Prop::parse(self.alt(), self.make_root()?) }))
    }

    /// Attempt to find a child of the current [`Node`] with the given name.
    ///
    /// For more details on what constitutes a node name which can be
    /// searchable, see [`IntoSearchableNodeName`].
    #[inline]
    #[track_caller]
    pub fn child<N>(&self, name: N) -> P::Output<Option<Node<'a, P>>>
    where
        N: IntoSearchableNodeName<'a>,
    {
        P::to_output(crate::tryblock!({ self.fallible().children()?.find(name).map(|o| o.map(|n| n.alt())) }))
    }

    /// Returns [`NodeChildren`] which allows searching and iterating over this
    /// node's children.
    #[inline]
    #[track_caller]
    pub fn children(&self) -> P::Output<NodeChildren<'a, P>> {
        P::to_output(tryblock!({
            let mut parser = P::new(&self.this.0, self.strings, self.structs);
            parser.advance_cstr()?;

            loop {
                match parser.peek_token() {
                    Ok(BigEndianToken::PROP) => parser.parse_raw_property()?,
                    Ok(BigEndianToken::BEGIN_NODE) => break,
                    Ok(_) | Err(FdtError::ParseError(ParseError::UnexpectedEndOfData)) => break,
                    Err(e) => return Err(e),
                };
            }

            Ok(NodeChildren {
                data: parser.data(),
                parent: self.this,
                strings: self.strings,
                structs: self.structs,
                _mode: core::marker::PhantomData,
            })
        }))
    }

    /// Attempt to retrieve the parent for this node. Note that this
    #[inline]
    pub fn parent(&self) -> Option<Self> {
        self.parent.map(|parent| Self {
            this: parent,
            parent: None,
            strings: self.strings,
            structs: self.structs,
            _mode: core::marker::PhantomData,
        })
    }
}

impl<'a, P: ParserWithMode<'a>> AsNode<'a, P> for Node<'a, P> {
    fn as_node(&self) -> Node<'a, P> {
        *self
    }
}

impl<'a, P: ParserWithMode<'a>> core::fmt::Debug for Node<'a, P>
where
    P::Output<NodeName<'a>>: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Node").field("name", &self.name()).finish_non_exhaustive()
    }
}

impl<'a, P: ParserWithMode<'a>> Clone for Node<'a, P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, P: ParserWithMode<'a>> Copy for Node<'a, P> {}

/// Newtype around a slice of raw node data.
#[repr(transparent)]
pub struct RawNode<Granularity = u32>([Granularity]);

impl<Granularity> RawNode<Granularity> {
    pub(crate) fn new(data: &[Granularity]) -> &Self {
        // SAFETY: the representation of `Self` and `data` are the same
        unsafe { core::mem::transmute(data) }
    }

    pub(crate) fn as_slice(&self) -> &[Granularity] {
        // SAFETY: the representation of `Self` and `data` are the same
        unsafe { core::mem::transmute(self) }
    }
}

/// Allows for searching and iterating over all of the properties of a given
/// [`Node`].
pub struct NodeProperties<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    data: &'a [<P as Parser<'a>>::Granularity],
    strings: StringsBlock<'a>,
    structs: StructsBlock<'a, <P as Parser<'a>>::Granularity>,
    _mode: core::marker::PhantomData<*mut P>,
}

impl<'a, P: ParserWithMode<'a>> NodeProperties<'a, P> {
    pub(crate) fn alt<P2: ParserWithMode<'a, Granularity = P::Granularity>>(self) -> NodeProperties<'a, P2> {
        NodeProperties {
            data: self.data,
            strings: self.strings,
            structs: self.structs,
            _mode: core::marker::PhantomData,
        }
    }

    /// Create an iterator over the properties in the [`Node`].
    #[inline(always)]
    pub fn iter(self) -> NodePropertiesIter<'a, P> {
        NodePropertiesIter { properties: self.alt(), _mode: core::marker::PhantomData }
    }

    #[track_caller]
    pub(crate) fn advance(&mut self) -> P::Output<Option<NodeProperty<'a>>> {
        let mut parser = P::new(self.data, self.strings, self.structs);

        match parser.peek_token() {
            Ok(BigEndianToken::PROP) => {}
            Ok(BigEndianToken::BEGIN_NODE) | Ok(BigEndianToken::END_NODE) => return P::to_output(Ok(None)),
            Ok(_) => {
                return P::to_output(Err(ParseError::UnexpectedToken.into()));
            }
            Err(FdtError::ParseError(ParseError::UnexpectedEndOfData)) => return P::to_output(Ok(None)),
            Err(e) => return P::to_output(Err(e)),
        }

        P::to_output(tryblock!({
            match parser.parse_raw_property() {
                Ok((name_offset, data)) => {
                    self.data = parser.data();

                    Ok(Some(NodeProperty::new(self.strings.offset_at(name_offset)?, data)))
                }
                Err(FdtError::ParseError(ParseError::UnexpectedEndOfData)) => Ok(None),
                Err(e) => return Err(e),
            }
        }))
    }

    /// Attempt to find a property with the provided name.
    #[inline]
    #[track_caller]
    pub fn find(&self, name: &str) -> P::Output<Option<NodeProperty<'a>>> {
        let this: NodeProperties<'a, (P::Parser, NoPanic)> = NodeProperties {
            data: self.data,
            strings: self.strings,
            structs: self.structs,
            _mode: core::marker::PhantomData,
        };

        P::to_output(
            this.iter()
                .find_map(|p| match p {
                    Err(e) => Some(Err(e)),
                    Ok(p) => (p.name == name).then_some(Ok(p)),
                })
                .transpose(),
        )
    }
}

impl<'a, P: ParserWithMode<'a>> Clone for NodeProperties<'a, P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, P: ParserWithMode<'a>> Copy for NodeProperties<'a, P> {}

impl<'a, P: ParserWithMode<'a>> IntoIterator for NodeProperties<'a, P> {
    type IntoIter = NodePropertiesIter<'a, P>;
    type Item = P::Output<NodeProperty<'a>>;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// See [`NodeProperties::iter`].
#[derive(Clone)]
pub struct NodePropertiesIter<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    properties: NodeProperties<'a, (P::Parser, NoPanic)>,
    _mode: core::marker::PhantomData<*mut P>,
}

impl<'a, P: ParserWithMode<'a>> Iterator for NodePropertiesIter<'a, P> {
    type Item = P::Output<NodeProperty<'a>>;

    #[track_caller]
    fn next(&mut self) -> Option<Self::Item> {
        // This is a manual impl of `map` because we need the panic location to
        // be the caller if `P::to_output` panics
        #[allow(clippy::manual_map)]
        match self.properties.advance().transpose() {
            Some(output) => Some(P::to_output(output)),
            None => None,
        }
    }
}

/// Generic node property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeProperty<'a> {
    /// Property name.
    pub name: &'a str,
    /// Raw property value.
    pub value: &'a [u8],
}

impl<'a> NodeProperty<'a> {
    #[inline(always)]
    pub fn new(name: &'a str, value: &'a [u8]) -> Self {
        Self { name, value }
    }

    /// Attempt to convert this property's value to the specified
    /// [`PropertyValue`] type.
    #[inline(always)]
    pub fn as_value<V: PropertyValue<'a>>(&self) -> Result<V, InvalidPropertyValue> {
        V::parse(self.value)
    }
}

/// Allows for searching and iterating over the children of a given [`Node`].
pub struct NodeChildren<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    data: &'a [<P as Parser<'a>>::Granularity],
    parent: &'a RawNode<<P as Parser<'a>>::Granularity>,
    strings: StringsBlock<'a>,
    structs: StructsBlock<'a, <P as Parser<'a>>::Granularity>,
    _mode: core::marker::PhantomData<*mut P>,
}

impl<'a, P: ParserWithMode<'a>> NodeChildren<'a, P> {
    /// Create an iterator over the [`Node`]'s children.
    #[inline(always)]
    pub fn iter(&self) -> NodeChildrenIter<'a, P> {
        NodeChildrenIter {
            children: NodeChildren {
                data: self.data,
                parent: self.parent,
                strings: self.strings,
                structs: self.structs,
                _mode: core::marker::PhantomData,
            },
        }
    }

    #[inline]
    pub(crate) fn advance(&mut self) -> P::Output<Option<Node<'a, P>>> {
        let mut parser = P::new(self.data, self.strings, self.structs);

        match parser.peek_token() {
            Ok(BigEndianToken::BEGIN_NODE) => {}
            Ok(BigEndianToken::END_NODE) => return P::to_output(Ok(None)),
            Ok(_) => return P::to_output(Err(ParseError::UnexpectedToken.into())),
            Err(FdtError::ParseError(ParseError::UnexpectedEndOfData)) => return P::to_output(Ok(None)),
            Err(e) => return P::to_output(Err(e)),
        }

        P::to_output(match parser.parse_node(Some(self.parent)) {
            Ok(node) => {
                self.data = parser.data();

                Ok(Some(node))
            }
            Err(FdtError::ParseError(ParseError::UnexpectedEndOfData)) => Ok(None),
            Err(e) => Err(e),
        })
    }

    /// Attempt to find the first child matching the provided name, see
    /// [`IntoSearchableNodeName`] for more details. If the name lacks a unit
    /// address, unit addresses on the children will be ignored when checking if
    /// the name matches.
    #[inline]
    #[track_caller]
    pub fn find<'n, N>(&self, name: N) -> P::Output<Option<Node<'a, P>>>
    where
        N: IntoSearchableNodeName<'n>,
    {
        let this: NodeChildren<(P::Parser, NoPanic)> = NodeChildren {
            data: self.data,
            parent: self.parent,
            strings: self.strings,
            structs: self.structs,
            _mode: core::marker::PhantomData,
        };

        let name = name.into_searchable_node_name();
        P::to_output(
            this.iter()
                .find_map(|n| match n {
                    Err(e) => Some(Err(e)),
                    Ok(node) => match node.name() {
                        Err(e) => Some(Err(e)),
                        Ok(nn) => match name {
                            SearchableNodeName::Base(base) => (nn.name == base).then_some(Ok(node)),
                            SearchableNodeName::WithUnitAddress(snn) => (nn == snn).then_some(Ok(node)),
                        },
                    },
                })
                .map(|n| n.map(Node::alt))
                .transpose(),
        )
    }
}

impl<'a, P: ParserWithMode<'a>> Clone for NodeChildren<'a, P> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, P: ParserWithMode<'a>> Copy for NodeChildren<'a, P> {}

impl<'a, P: ParserWithMode<'a>> IntoIterator for NodeChildren<'a, P> {
    type IntoIter = NodeChildrenIter<'a, P>;
    type Item = P::Output<Node<'a, P>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// See [`NodeChildren::iter`].
#[derive(Clone)]
pub struct NodeChildrenIter<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    children: NodeChildren<'a, (P::Parser, NoPanic)>,
}

impl<'a, P: ParserWithMode<'a>> Iterator for NodeChildrenIter<'a, P> {
    type Item = P::Output<Node<'a, P>>;

    #[track_caller]
    fn next(&mut self) -> Option<Self::Item> {
        // This is a manual impl of `map` because we need the panic location to
        // be the caller if `P::to_output` panics
        #[allow(clippy::manual_map)]
        match self.children.advance().transpose() {
            Some(output) => Some(P::to_output(output.map(Node::alt))),
            None => None,
        }
    }
}
