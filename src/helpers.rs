use crate::{
    nodes::{root::Root, Node},
    parsing::{aligned::AlignedParser, unaligned::UnalignedParser, NoPanic, Panic, ParserWithMode},
};

pub type FallibleParser<'a, P> = (<P as ParserWithMode<'a>>::Parser, NoPanic);
pub type FallibleNode<'a, P> = Node<'a, FallibleParser<'a, P>>;
pub type FallibleRoot<'a, P> = Root<'a, FallibleParser<'a, P>>;

pub type AlignedFallibleNode<'a> = Node<'a, (AlignedParser<'a>, NoPanic)>;
pub type UnalignedFallibleNode<'a> = Node<'a, (UnalignedParser<'a>, NoPanic)>;

pub type AlignedInfallibleNode<'a> = Node<'a, (AlignedParser<'a>, Panic)>;
pub type UnalignedInfallibleNode<'a> = Node<'a, (UnalignedParser<'a>, Panic)>;
