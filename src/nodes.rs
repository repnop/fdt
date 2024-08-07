pub mod aliases;
pub mod chosen;
pub mod cpus;
pub mod memory;
pub mod root;

use root::Root;

use crate::{
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
    fn as_node(&self) -> Node<'a, P>;
}

#[derive(Debug, Clone, Copy)]
pub enum SearchableNodeName<'a> {
    Base(&'a str),
    WithUnitAddress(NodeName<'a>),
}

pub trait IntoSearchableNodeName<'a>: Sized + crate::sealed::Sealed {
    fn into_searchable_node_name(self) -> SearchableNodeName<'a>;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NodeName<'a> {
    pub name: &'a str,
    pub unit_address: Option<&'a str>,
}

impl<'a> NodeName<'a> {
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

pub type FallibleNode<'a, P> = Node<'a, (<P as ParserWithMode<'a>>::Parser, NoPanic)>;
pub type FallibleRoot<'a, P> = Root<'a, (<P as ParserWithMode<'a>>::Parser, NoPanic)>;

pub struct Node<'a, P: ParserWithMode<'a>> {
    pub(crate) this: &'a RawNode<<P as Parser<'a>>::Granularity>,
    pub(crate) parent: Option<&'a RawNode<<P as Parser<'a>>::Granularity>>,
    pub(crate) strings: StringsBlock<'a>,
    pub(crate) structs: StructsBlock<'a, <P as Parser<'a>>::Granularity>,
    pub(crate) _mode: core::marker::PhantomData<*mut P>,
}

impl<'a, P: ParserWithMode<'a>> Node<'a, P> {
    /// Change the type of this node's [`PanicMode`] to [`NoPanic`]
    #[inline(always)]
    pub(crate) fn fallible(self) -> FallibleNode<'a, P> {
        self.alt()
    }

    /// Helper function for changing the [`PanicMode`] of this node
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

    #[inline]
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

    #[inline(always)]
    pub fn reg(&self) -> P::Output<Option<Reg<'a>>> {
        self.property::<Reg<'a>>()
    }

    pub fn ranges(&self) -> P::Output<Option<Ranges<'a>>> {
        self.property()
    }

    #[inline]
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

    #[inline]
    pub fn raw_property(&self, name: &str) -> P::Output<Option<NodeProperty<'a>>> {
        P::to_output(tryblock!({
            let this = self.fallible();
            this.properties()?.find(name)
        }))
    }

    #[track_caller]
    pub fn property<Prop: Property<'a, P>>(&self) -> P::Output<Option<Prop>> {
        P::to_output(crate::tryblock!({ Prop::parse(self.alt(), self.make_root()?) }))
    }

    #[inline]
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

    pub fn iter(self) -> NodePropertiesIter<'a, P> {
        NodePropertiesIter { properties: self.alt(), _mode: core::marker::PhantomData }
    }

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

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct NodeProperty<'a> {
    name: &'a str,
    value: &'a [u8],
}

impl<'a> NodeProperty<'a> {
    pub fn new(name: &'a str, value: &'a [u8]) -> Self {
        Self { name, value }
    }

    pub fn name(&self) -> &'a str {
        self.name
    }

    pub fn value(&self) -> &'a [u8] {
        self.value
    }

    pub fn as_value<V: PropertyValue<'a>>(&self) -> Result<V, InvalidPropertyValue> {
        V::parse(self.value)
    }
}

pub struct NodeChildren<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    data: &'a [<P as Parser<'a>>::Granularity],
    parent: &'a RawNode<<P as Parser<'a>>::Granularity>,
    strings: StringsBlock<'a>,
    structs: StructsBlock<'a, <P as Parser<'a>>::Granularity>,
    _mode: core::marker::PhantomData<*mut P>,
}

impl<'a, P: ParserWithMode<'a>> NodeChildren<'a, P> {
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
