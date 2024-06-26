use core::ffi::CStr;

use crate::{
    parsing::{
        aligned::AlignedParser, BigEndianToken, BigEndianU32, NoPanic, Panic, PanicMode,
        ParseError, Parser, ParserWithMode, StringsBlock,
    },
    properties::Property,
    FdtError,
};

#[macro_export]
#[doc(hidden)]
macro_rules! tryblock {
    ($($ts:tt)+) => {{
        (|| -> Result<_, $crate::FdtError> {
            $($ts)+
        })()
    }};
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
            Some((base, unit_address)) => SearchableNodeName::WithUnitAddress(NodeName {
                name: base,
                unit_address: Some(unit_address),
            }),
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

pub struct Node<'a, P: ParserWithMode<'a>> {
    pub(crate) this: &'a RawNode<<P as Parser<'a>>::Granularity>,
    pub(crate) parent: Option<&'a RawNode<<P as Parser<'a>>::Granularity>>,
    pub(crate) strings: StringsBlock<'a>,
    pub(crate) _mode: core::marker::PhantomData<*mut P>,
}

impl<'a, P: ParserWithMode<'a>> Node<'a, P> {
    #[inline(always)]
    pub(crate) fn fallible(self) -> Node<'a, (P::Parser, NoPanic)> {
        Node {
            this: self.this,
            parent: self.parent,
            strings: self.strings,
            _mode: core::marker::PhantomData,
        }
    }

    #[inline(always)]
    pub(crate) fn alt<P2: ParserWithMode<'a, Granularity = P::Granularity>>(self) -> Node<'a, P2> {
        Node {
            this: self.this,
            parent: self.parent,
            strings: self.strings,
            _mode: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn name(&self) -> <P as PanicMode>::Output<NodeName<'a>> {
        P::to_output(
            P::new(&self.this.0, self.strings.0)
                .advance_cstr()
                .and_then(|s| {
                    s.to_str().map_err(|_| FdtError::ParseError(ParseError::InvalidCStrValue))
                })
                .map(|s| {
                    if s.is_empty() {
                        return NodeName { name: "/", unit_address: None };
                    }

                    let (name, unit_address) = s.split_once('@').unzip();
                    NodeName { name: name.unwrap_or(s), unit_address }
                }),
        )
    }

    #[inline]
    pub fn properties(&self) -> P::Output<NodeProperties<'a, P>> {
        let mut parser = P::new(&self.this.0, self.strings.0);
        let res = parser.advance_cstr();

        P::to_output(res.map(|_| NodeProperties {
            data: parser.data(),
            strings: self.strings,
            _mode: core::marker::PhantomData,
        }))
    }

    #[inline]
    pub fn raw_property(&self, name: &str) -> P::Output<Option<NodeProperty<'a>>> {
        P::to_output(tryblock! {
            P::to_result(P::to_result(self.properties())?.find(name))
        })
    }

    #[track_caller]
    pub fn property<Prop: Property<'a>>(&self) -> P::Output<Option<Prop>> {
        P::to_output(Prop::parse(self.fallible()))
    }

    #[inline]
    pub fn children(&self) -> P::Output<NodeChildren<'a, P>> {
        P::to_output(tryblock! {
            let mut parser = P::new(&self.this.0, self.strings.0);
            parser.advance_cstr()?;
            while let BigEndianToken::PROP = parser.peek_token()? {
                parser.parse_raw_property()?;
            }

            Ok(NodeChildren {
                data: parser.data(),
                parent: self.this,
                strings: self.strings,
                _mode: core::marker::PhantomData,
            })
        })
    }

    #[inline]
    pub fn parent(&self) -> Option<Self> {
        self.parent.map(|parent| Self {
            this: parent,
            parent: None,
            strings: self.strings,
            _mode: core::marker::PhantomData,
        })
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
    _mode: core::marker::PhantomData<*mut P>,
}

impl<'a, P: ParserWithMode<'a>> NodeProperties<'a, P> {
    pub fn iter(self) -> NodePropertiesIter<'a, P> {
        NodePropertiesIter { properties: self }
    }

    pub fn advance(&mut self) -> P::Output<Option<NodeProperty<'a>>> {
        let mut parser = P::new(self.data, self.strings.0);

        match parser.peek_token() {
            Ok(BigEndianToken::PROP) => {}
            Ok(BigEndianToken::BEGIN_NODE) | Ok(BigEndianToken::END_NODE) => {
                return P::to_output(Ok(None))
            }
            Ok(_) => {
                return P::to_output(Err(ParseError::UnexpectedToken.into()));
            }
            Err(FdtError::ParseError(ParseError::UnexpectedEndOfData)) => {
                return P::to_output(Ok(None))
            }
            Err(e) => return P::to_output(Err(e)),
        }

        P::to_output(tryblock! {
            match parser.parse_raw_property() {
                Ok((name_offset, data)) => {
                    self.data = parser.data();

                    Ok(Some(NodeProperty::new(self.strings.offset_at(name_offset)?, data)))
                }
                Err(FdtError::ParseError(ParseError::UnexpectedEndOfData)) => Ok(None),
                Err(e) => return Err(e),
            }
        })
    }

    pub fn find(&self, name: &str) -> P::Output<Option<NodeProperty<'a>>> {
        let this: NodeProperties<'a, (P::Parser, NoPanic)> = NodeProperties {
            data: self.data,
            strings: self.strings,
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
    properties: NodeProperties<'a, P>,
}

impl<'a, P: ParserWithMode<'a>> Iterator for NodePropertiesIter<'a, P> {
    type Item = P::Output<NodeProperty<'a>>;

    #[track_caller]
    fn next(&mut self) -> Option<Self::Item> {
        P::transpose(self.properties.advance())
    }
}

pub struct InvalidPropertyValue;

impl From<InvalidPropertyValue> for FdtError {
    fn from(_: InvalidPropertyValue) -> Self {
        FdtError::InvalidPropertyValue
    }
}

pub trait Value<'a>: Sized {
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue>;
}

impl<'a> Value<'a> for u32 {
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue> {
        match value {
            [a, b, c, d] => Ok(u32::from_be_bytes([*a, *b, *c, *d])),
            _ => Err(InvalidPropertyValue),
        }
    }
}

impl<'a> Value<'a> for BigEndianU32 {
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue> {
        match value {
            [a, b, c, d] => Ok(BigEndianU32::from_be(u32::from_ne_bytes([*a, *b, *c, *d]))),
            _ => Err(InvalidPropertyValue),
        }
    }
}

impl<'a> Value<'a> for &'a CStr {
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue> {
        CStr::from_bytes_until_nul(value).map_err(|_| InvalidPropertyValue)
    }
}

impl<'a> Value<'a> for &'a str {
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue> {
        core::str::from_utf8(value)
            .map(|s| s.trim_end_matches('\0'))
            .map_err(|_| InvalidPropertyValue)
    }
}

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

    pub fn to<V: Value<'a>>(&self) -> Result<V, InvalidPropertyValue> {
        V::parse(self.value)
    }
}

pub struct NodeChildren<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    data: &'a [<P as Parser<'a>>::Granularity],
    parent: &'a RawNode<<P as Parser<'a>>::Granularity>,
    strings: StringsBlock<'a>,
    _mode: core::marker::PhantomData<*mut P>,
}

impl<'a, P: ParserWithMode<'a>> NodeChildren<'a, P> {
    pub fn iter(self) -> NodeChildrenIter<'a, P> {
        NodeChildrenIter { children: self }
    }

    pub fn advance(&mut self) -> P::Output<Option<Node<'a, P>>> {
        let mut parser = P::new(self.data, self.strings.0);

        match parser.peek_token() {
            Ok(BigEndianToken::BEGIN_NODE) => {}
            Ok(BigEndianToken::END_NODE) => return P::to_output(Ok(None)),
            Ok(_) => return P::to_output(Err(ParseError::UnexpectedToken.into())),
            Err(FdtError::ParseError(ParseError::UnexpectedEndOfData)) => {
                return P::to_output(Ok(None))
            }
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
        P: 'a,
    {
        let this: NodeChildren<'a, (P::Parser, NoPanic)> = NodeChildren {
            data: self.data,
            parent: self.parent,
            strings: self.strings,
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
                            SearchableNodeName::WithUnitAddress(snn) => {
                                (nn == snn).then_some(Ok(node))
                            }
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
    children: NodeChildren<'a, P>,
}

impl<'a, P: ParserWithMode<'a> + 'a> Iterator for NodeChildrenIter<'a, P> {
    type Item = P::Output<Node<'a, P>>;

    #[track_caller]
    fn next(&mut self) -> Option<Self::Item> {
        P::transpose(self.children.advance())
    }
}
