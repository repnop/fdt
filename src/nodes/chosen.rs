// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    parsing::{aligned::AlignedParser, Panic, ParseError, ParserWithMode},
    FdtError,
};

use super::FallibleNode;

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
    /// Contains the bootargs, if they exist
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

    /// Looks up the `stdout-path` property and returns the [`StdInOutPath`]
    /// representing the path. The path may be an alias and require being
    /// resolved with [`Alias::resolve`] before being used in conjunction with
    /// [`Root::find_node`]. For more information about the path parameters, see
    /// [`StdInOutPath::params`].
    #[track_caller]
    pub fn stdout(self) -> P::Output<Option<StdInOutPath<'a>>> {
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

    /// Looks up the `stdin-path` property and returns the [`StdInOutPath`]
    /// representing the path. The path may be an alias and require being
    /// resolved with [`Alias::resolve`] before being used in conjunction with
    /// [`Root::find_node`]. For more information about the path parameters, see
    /// [`StdInOutPath::params`].
    #[track_caller]
    pub fn stdin(self) -> P::Output<Option<StdInOutPath<'a>>> {
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
    /// let stdout = chosen.stdout().unwrap();
    /// let stdin = chosen.stdin().unwrap();
    ///
    /// assert_eq!((stdout.path(), stdout.params()), ("/soc/uart@10000000", Some("115200")));
    /// assert_eq!((stdin.path(), stdin.params()), ("/soc/uart@10000000", None));
    /// ```
    pub fn params(&self) -> Option<&'a str> {
        self.params
    }
}
