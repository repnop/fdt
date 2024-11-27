// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::{FallibleNode, FallibleParser, FallibleRoot, Node};
use crate::{
    parsing::{aligned::AlignedParser, Panic, ParseError, ParserWithMode},
    FdtError,
};

/// [Devicetree 3.6. `/chosen`
/// Node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#chosen-node)
///
/// The `/chosen` node does not represent a real device in the system but
/// describes parameters chosen or specified by the system firmware at run time.
/// It shall be a child of the root node.
pub struct Chosen<'a, P: ParserWithMode<'a> = (AlignedParser<'a>, Panic)> {
    pub(crate) node: FallibleNode<'a, P>,
}

impl<'a, P: ParserWithMode<'a>> Chosen<'a, P> {
    /// [Devicetree 3.6. `/chosen`
    /// Node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#chosen-node)
    ///
    /// A string that specifies the boot arguments for the client program. The
    /// value could potentially be a null string if no boot arguments are
    /// required.
    #[track_caller]
    pub fn bootargs(self) -> P::Output<Option<&'a str>> {
        P::to_output(crate::tryblock!({
            for prop in self.node.properties()?.into_iter().flatten() {
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

    /// Like [`Chosen::stdout_path`] but also attempts to resolve the path (also
    /// attempts to resolve the path to an alias if: the path does not look like
    /// a devicetree path, or the path is not found), and returns the stdout
    /// parameters along with the node, if it was successfully resolved.
    ///
    /// For more information on the `stdout-path` property, see
    /// [`Chosen::stdout_path`].
    #[allow(clippy::type_complexity)]
    #[track_caller]
    pub fn stdout(self) -> P::Output<Option<Stdout<'a, P>>> {
        P::to_output(crate::tryblock!({
            let this: Chosen<'a, FallibleParser<'a, P>> = Chosen { node: self.node };
            let Some(stdout) = this.stdout_path()? else { return Ok(None) };
            let root: FallibleRoot<'a, P> = this.node.make_root()?;

            let node = match stdout.path().contains('/') {
                true => root.find_node(stdout.path())?,
                false => None,
            };

            match node {
                Some(node) => Ok(Some(Stdout { node: node.alt(), params: stdout.params() })),
                None => {
                    let Some(aliases) = root.aliases()? else { return Ok(None) };
                    match aliases.resolve(stdout.path())? {
                        Some(node) => Ok(Some(Stdout { node: node.alt(), params: stdout.params() })),
                        None => Ok(None),
                    }
                }
            }
        }))
    }

    /// [Devicetree 3.6. `/chosen`
    /// Node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#chosen-node)
    ///
    /// A string that specifies the full path to the node representing the
    /// device to be used for boot console output. If the character ":" is
    /// present in the value it terminates the path. The value may be an alias.
    /// If the `stdin-path` property is not specified, `stdout-path` should be
    /// assumed to define the input device.
    #[track_caller]
    pub fn stdout_path(self) -> P::Output<Option<StdInOutPath<'a>>> {
        P::to_output(crate::tryblock!({
            self.node
                .properties()?
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

    /// Like [`Chosen::stdin_path`] but also attempts to resolve the path (also
    /// attempts to resolve the path to an alias if: the path does not look like
    /// a devicetree path, or the path is not found), and returns the stdin
    /// parameters along with the node, if it was successfully resolved.
    ///
    /// For more information on the `stdin-path` property, see
    /// [`Chosen::stdin_path`].
    #[allow(clippy::type_complexity)]
    #[track_caller]
    pub fn stdin(self) -> P::Output<Option<Stdin<'a, P>>> {
        P::to_output(crate::tryblock!({
            let this: Chosen<'a, FallibleParser<'a, P>> = Chosen { node: self.node };
            let Some(stdin) = this.stdin_path()? else { return Ok(None) };
            let root: FallibleRoot<'a, P> = this.node.make_root()?;

            let node = match stdin.path().contains('/') {
                true => root.find_node(stdin.path())?,
                false => None,
            };

            match node {
                Some(node) => Ok(Some(Stdin { node: node.alt(), params: stdin.params() })),
                None => {
                    let Some(aliases) = root.aliases()? else { return Ok(None) };
                    match aliases.resolve(stdin.path())? {
                        Some(node) => Ok(Some(Stdin { node: node.alt(), params: stdin.params() })),
                        None => Ok(None),
                    }
                }
            }
        }))
    }

    /// [Devicetree 3.6. `/chosen`
    /// Node](https://devicetree-specification.readthedocs.io/en/latest/chapter3-devicenodes.html#chosen-node)
    ///
    /// A string that specifies the full path to the node representing the
    /// device to be used for boot console input. If the character ":" is
    /// present in the value it terminates the path. The value may be an alias.
    #[track_caller]
    pub fn stdin_path(self) -> P::Output<Option<StdInOutPath<'a>>> {
        P::to_output(crate::tryblock!({
            self.node
                .properties()?
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

/// See [`Chosen::stdin`].
pub struct Stdin<'a, P: ParserWithMode<'a>> {
    pub node: Node<'a, P>,
    pub params: Option<&'a str>,
}

impl<'a, P: ParserWithMode<'a>> core::fmt::Debug for Stdin<'a, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut debug_struct = f.debug_struct("Stdin");
        let debug_struct = match self.node.fallible().name() {
            Ok(name) => debug_struct.field("node", &name),
            Err(e) => debug_struct.field("node", &Err::<(), _>(e)),
        };

        debug_struct.field("params", &self.params).finish()
    }
}

/// See [`Chosen::stdout`].
pub struct Stdout<'a, P: ParserWithMode<'a>> {
    pub node: Node<'a, P>,
    pub params: Option<&'a str>,
}

impl<'a, P: ParserWithMode<'a>> core::fmt::Debug for Stdout<'a, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut debug_struct = f.debug_struct("Stdin");
        let debug_struct = match self.node.fallible().name() {
            Ok(name) => debug_struct.field("node", &name),
            Err(e) => debug_struct.field("node", &Err::<(), _>(e)),
        };

        debug_struct.field("params", &self.params).finish()
    }
}

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
    /// # let fdt = fdt::Fdt::new_unaligned(include_bytes!("../../dtb/test.dtb")).unwrap();
    /// # let chosen = fdt.root().chosen();
    /// let stdout = chosen.stdout_path().unwrap();
    /// let stdin = chosen.stdin_path().unwrap();
    ///
    /// assert_eq!((stdout.path(), stdout.params()), ("/soc/uart@10000000", Some("115200")));
    /// assert_eq!((stdin.path(), stdin.params()), ("/soc/uart@10000000", None));
    /// ```
    pub fn params(&self) -> Option<&'a str> {
        self.params
    }
}
