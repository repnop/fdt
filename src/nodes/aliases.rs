use super::{AsNode, Node, NodePropertiesIter};
use crate::{
    helpers::{FallibleNode, FallibleParser},
    parsing::{NoPanic, ParserWithMode},
};

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

    /// Create an iterator over all of the available aliases
    pub fn iter(self) -> P::Output<AllAliasesIter<'a, P>> {
        P::to_output(crate::tryblock!({ Ok(AllAliasesIter { properties: self.node.properties()?.iter() }) }))
    }
}

impl<'a, P: ParserWithMode<'a>> AsNode<'a, P> for Aliases<'a, P> {
    fn as_node(&self) -> Node<'a, P> {
        self.node.alt()
    }
}

pub struct AllAliasesIter<'a, P: ParserWithMode<'a>> {
    properties: NodePropertiesIter<'a, FallibleParser<'a, P>>,
}

impl<'a, P> Iterator for AllAliasesIter<'a, P>
where
    P: ParserWithMode<'a>,
{
    type Item = P::Output<(&'a str, &'a str)>;
    #[track_caller]
    fn next(&mut self) -> Option<Self::Item> {
        Some(P::to_output(match self.properties.next() {
            Some(Ok(prop)) => crate::tryblock!({ Ok((prop.name(), prop.as_value::<&'a str>()?)) }),
            Some(Err(e)) => Err(e),
            None => return None,
        }))
    }
}
