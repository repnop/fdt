use crate::{
    parsing::{
        self, BigEndianToken, Panic, PanicMode, ParseError, Parser, ParserForSize, StringsBlock,
    },
    properties::Property,
};

#[macro_export]
#[doc(hidden)]
macro_rules! tryblock {
    ($($ts:tt)+) => {{
        (|| -> Result<_, $crate::parsing::ParseError> {
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
        match self.split_once('@') {
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

pub struct Node<'a, Granularity: ParserForSize = u32, Mode: PanicMode = Panic> {
    pub(crate) this: &'a RawNode<Granularity>,
    pub(crate) parent: Option<&'a RawNode<Granularity>>,
    pub(crate) strings: StringsBlock<'a>,
    pub(crate) _mode: core::marker::PhantomData<*mut Mode>,
}

impl<'a, Granularity: ParserForSize, Mode: PanicMode> Node<'a, Granularity, Mode> {
    #[inline]
    pub(crate) fn new(
        this: &'a RawNode<Granularity>,
        parent: Option<&'a RawNode<Granularity>>,
        strings: StringsBlock<'a>,
    ) -> Self {
        Self { this, parent, strings, _mode: core::marker::PhantomData }
    }

    #[inline]
    pub fn name(&self) -> <Mode as PanicMode>::Output<NodeName<'a>> {
        <Mode as PanicMode>::to_output(
            <<Granularity as ParserForSize>::Parser<'a> as Parser<'a>>::new(
                &self.this.0,
                self.strings.0,
            )
            .advance_cstr()
            .and_then(|s| s.to_str().map_err(|_| ParseError::InvalidCStrValue))
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
    pub fn properties(&self) -> <Mode as PanicMode>::Output<NodeProperties<'a, Granularity>> {
        let mut parser = <<Granularity as ParserForSize>::Parser<'a> as Parser<'a>>::new(
            &self.this.0,
            self.strings.0,
        );
        let res = parser.advance_cstr();

        <Mode as PanicMode>::to_output(res.map(|_| NodeProperties {
            data: parser.data(),
            strings: self.strings,
            _mode: core::marker::PhantomData,
        }))
    }

    #[inline]
    pub fn raw_property(
        &self,
        name: &str,
    ) -> <Mode as PanicMode>::Output<Option<NodeProperty<'a>>> {
        <Mode as PanicMode>::to_output(tryblock! {
            Ok(<Mode as PanicMode>::to_result(self.properties())?.find(name))
        })
    }

    pub fn property<P: Property<'a>>(&self) -> <Mode as PanicMode>::Output<Option<P>> {
        <Mode as PanicMode>::to_output(P::parse(*self))
    }

    #[inline]
    pub fn children(&self) -> <Mode as PanicMode>::Output<NodeChildren<'a, Granularity, Mode>> {
        <Mode as PanicMode>::to_output(tryblock! {
            let mut parser =
                <<Granularity as ParserForSize>::Parser<'a> as Parser<'a>>::new(&self.this.0, self.strings.0);
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

impl<'a, Granularity: ParserForSize, Mode: PanicMode> core::fmt::Debug
    for Node<'a, Granularity, Mode>
where
    <Mode as PanicMode>::Output<NodeName<'a>>: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Node").field("name", &self.name()).finish_non_exhaustive()
    }
}

impl<'a, Granularity: ParserForSize, Mode: PanicMode> Clone for Node<'a, Granularity, Mode> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, Granularity: ParserForSize, Mode: PanicMode> Copy for Node<'a, Granularity, Mode> {}

#[repr(transparent)]
pub(crate) struct RawNode<Granularity: ParserForSize = u32>([Granularity]);

impl<Granularity: ParserForSize> RawNode<Granularity> {
    pub(crate) fn new(data: &[Granularity]) -> &Self {
        // SAFETY: the representation of `Self` and `data` are the same
        unsafe { core::mem::transmute(data) }
    }

    pub(crate) fn as_slice(&self) -> &[Granularity] {
        // SAFETY: the representation of `Self` and `data` are the same
        unsafe { core::mem::transmute(self) }
    }
}

#[derive(Debug)]
pub struct NodeProperties<'a, Granularity: ParserForSize = u32, Mode: PanicMode = Panic> {
    data: &'a [Granularity],
    strings: StringsBlock<'a>,
    _mode: core::marker::PhantomData<*mut Mode>,
}

impl<'a, Granularity: ParserForSize, Mode: PanicMode> NodeProperties<'a, Granularity, Mode> {
    pub fn iter(self) -> NodePropertiesIter<'a, Granularity, Mode> {
        NodePropertiesIter { properties: self }
    }

    pub fn advance(&mut self) -> <Mode as PanicMode>::Output<Option<NodeProperty<'a>>> {
        let mut parser = <<Granularity as ParserForSize>::Parser<'a> as Parser<'a>>::new(
            self.data,
            self.strings.0,
        );

        match parser.peek_token() {
            Ok(BigEndianToken::PROP) => {}
            Ok(BigEndianToken::BEGIN_NODE) => return <Mode as PanicMode>::to_output(Ok(None)),
            Ok(_) => return <Mode as PanicMode>::to_output(Err(ParseError::UnexpectedToken)),
            Err(ParseError::UnexpectedEndOfData) => {
                return <Mode as PanicMode>::to_output(Ok(None))
            }
            Err(e) => return <Mode as PanicMode>::to_output(Err(e)),
        }

        <Mode as PanicMode>::to_output(tryblock! {
            match parser.parse_raw_property() {
                Ok((name_offset, data)) => {
                    self.data = parser.data();

                    Ok(Some(NodeProperty::new(self.strings.offset_at(name_offset)?, data)))
                }
                Err(ParseError::UnexpectedEndOfData) => Ok(None),
                Err(e) => return Err(e),
            }
        })
    }

    pub fn find(&self, name: &str) -> <Mode as PanicMode>::Output<Option<NodeProperty<'a>>> {
        <Mode as PanicMode>::reverse_transpose(self.iter().find(|p| {
            <Mode as PanicMode>::ok_as_ref(p).map(|p| p.name == name).unwrap_or_default()
        }))
    }
}

impl<Granularity: ParserForSize, Mode: PanicMode> Clone for NodeProperties<'_, Granularity, Mode> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<Granularity: ParserForSize, Mode: PanicMode> Copy for NodeProperties<'_, Granularity, Mode> {}

impl<'a, Granularity: ParserForSize, Mode: PanicMode> IntoIterator
    for NodeProperties<'a, Granularity, Mode>
{
    type IntoIter = NodePropertiesIter<'a, Granularity, Mode>;
    type Item = <Mode as PanicMode>::Output<NodeProperty<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[derive(Debug, Clone)]
pub struct NodePropertiesIter<'a, Granularity: ParserForSize = u32, Mode: PanicMode = Panic> {
    properties: NodeProperties<'a, Granularity, Mode>,
}

impl<'a, Granularity: ParserForSize, Mode: PanicMode> Iterator
    for NodePropertiesIter<'a, Granularity, Mode>
{
    type Item = <Mode as PanicMode>::Output<NodeProperty<'a>>;

    #[track_caller]
    fn next(&mut self) -> Option<Self::Item> {
        <Mode as PanicMode>::transpose(self.properties.advance())
    }
}

pub trait Value<'a>: Sized {
    fn parse(value: &'a [u8]) -> Option<Self>;
}

impl<'a> Value<'a> for u32 {
    fn parse(value: &'a [u8]) -> Option<Self> {
        match value {
            [a, b, c, d, ..] => Some(u32::from_be_bytes([*a, *b, *c, *d])),
            _ => None,
        }
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

    pub fn to<V: Value<'a>>(&self) -> Option<V> {
        V::parse(self.value)
    }
}

pub struct NodeChildren<'a, Granularity: ParserForSize = u32, Mode: PanicMode = Panic> {
    data: &'a [Granularity],
    parent: &'a RawNode<Granularity>,
    strings: StringsBlock<'a>,
    _mode: core::marker::PhantomData<*mut Mode>,
}

impl<'a, Granularity: ParserForSize, Mode: PanicMode> NodeChildren<'a, Granularity, Mode> {
    pub fn iter(self) -> NodeChildrenIter<'a, Granularity, Mode> {
        NodeChildrenIter { children: self }
    }

    pub fn advance(&mut self) -> <Mode as PanicMode>::Output<Option<Node<'a, Granularity, Mode>>> {
        let mut parser = <<Granularity as ParserForSize>::Parser<'a> as Parser<'a>>::new(
            self.data,
            self.strings.0,
        );

        match parser.peek_token() {
            Ok(BigEndianToken::BEGIN_NODE) => {}
            Ok(BigEndianToken::END_NODE) => return <Mode as PanicMode>::to_output(Ok(None)),
            Ok(_) => return <Mode as PanicMode>::to_output(Err(ParseError::UnexpectedToken)),
            Err(ParseError::UnexpectedEndOfData) => {
                return <Mode as PanicMode>::to_output(Ok(None))
            }
            Err(e) => return <Mode as PanicMode>::to_output(Err(e)),
        }

        <Mode as PanicMode>::to_output(match parser.parse_node(Some(self.parent)) {
            Ok(node) => {
                self.data = parser.data();

                Ok(Some(node))
            }
            Err(ParseError::UnexpectedEndOfData) => Ok(None),
            Err(e) => Err(e),
        })
    }

    pub fn find<'n, N>(
        &self,
        name: N,
    ) -> <Mode as PanicMode>::Output<Option<Node<'a, Granularity, Mode>>>
    where
        N: IntoSearchableNodeName<'n>,
        Mode: 'a,
    {
        let name = name.into_searchable_node_name();
        <Mode as PanicMode>::reverse_transpose(self.iter().find(|n| {
            <Mode as PanicMode>::ok_as_ref(n)
                .and_then(|n| <Mode as PanicMode>::ok(n.name()))
                .map(|n| match name {
                    SearchableNodeName::Base(base) => n.name == base,
                    SearchableNodeName::WithUnitAddress(nn) => n == nn,
                })
                .unwrap_or_default()
        }))
    }
}

impl<'a, Granularity: ParserForSize, Mode: PanicMode> Clone
    for NodeChildren<'a, Granularity, Mode>
{
    fn clone(&self) -> Self {
        Self { _mode: self._mode, data: self.data, strings: self.strings, parent: self.parent }
    }
}

impl<'a, Granularity: ParserForSize, Mode: PanicMode> Copy for NodeChildren<'a, Granularity, Mode> {}

#[derive(Clone)]
pub struct NodeChildrenIter<'a, Granularity: ParserForSize = u32, Mode: PanicMode = Panic> {
    children: NodeChildren<'a, Granularity, Mode>,
}

impl<'a, Granularity: ParserForSize, Mode: PanicMode + 'a> Iterator
    for NodeChildrenIter<'a, Granularity, Mode>
{
    type Item = <Mode as PanicMode>::Output<Node<'a, Granularity, Mode>>;

    #[track_caller]
    fn next(&mut self) -> Option<Self::Item> {
        <Mode as PanicMode>::transpose(self.children.advance())
    }
}
