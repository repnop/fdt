use crate::{
    nodes::{root::Root, Node},
    parsing::{aligned::AlignedParser, unaligned::UnalignedParser, NoPanic, Panic, ParserWithMode},
};

/// Parser mode tuple which indicates the parser will not panic and return [`Result`]s instead.
pub type FallibleParser<'a, P> = (<P as ParserWithMode<'a>>::Parser, NoPanic);
/// A node using a fallible parser.
pub type FallibleNode<'a, P> = Node<'a, FallibleParser<'a, P>>;
/// Devicetree root which uses a fallible parser.
pub type FallibleRoot<'a, P> = Root<'a, FallibleParser<'a, P>>;

/// Indicates the underlying data is aligned to 4 bytes and the parser will
/// produce [`Result`]s instead of panicking.
pub type AlignedFallibleNode<'a> = Node<'a, (AlignedParser<'a>, NoPanic)>;
/// Indicates the underlying data is byte aligned and the parser will
/// produce [`Result`]s instead of panicking.
pub type UnalignedFallibleNode<'a> = Node<'a, (UnalignedParser<'a>, NoPanic)>;

/// Indicates the underlying data is aligned to 4 bytes and the parser will
/// panic if invalid devicetree data is encountered.
pub type AlignedInfallibleNode<'a> = Node<'a, (AlignedParser<'a>, Panic)>;
/// Indicates the underlying data is byte aligned and the parser will
/// panic if invalid devicetree data is encountered.
pub type UnalignedInfallibleNode<'a> = Node<'a, (UnalignedParser<'a>, Panic)>;
