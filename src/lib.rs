// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

//! # `fdt`
//!
//! A pure-Rust `#![no_std]` crate for parsing Flattened Devicetrees, with the
//! goal of having a very ergonomic and idiomatic API.
//!
//! [![crates.io](https://img.shields.io/crates/v/fdt.svg)](https://crates.io/crates/fdt)
//! [![Documentation](https://docs.rs/fdt/badge.svg)](https://docs.rs/fdt)
//! ![Build](https://github.com/repnop/fdt/actions/workflows/test.yml/badge.svg?branch=master&event=push)
//!
//! ## License
//!
//! This crate is licensed under the Mozilla Public License 2.0 (see the LICENSE
//! file).
//!
//! ## Example
//!
//! ```rust
//! static MY_FDT: &[u8] = include_bytes!("../dtb/test.dtb");
//!
//! fn main() {
//!     let fdt = fdt::Fdt::new_unaligned(MY_FDT).unwrap();
//!     let root = fdt.root();
//!
//!     println!("This is a devicetree representation of a {}", root.model());
//!     println!("...which is compatible with at least: {}", root.compatible().first());
//!     println!("...and has {} CPU(s)", root.cpus().iter().count());
//!     println!(
//!         "...and has at least one memory location at: {:#X}\n",
//!         root.memory().reg().iter::<u64, u64>().next().unwrap().unwrap().address
//!     );
//!
//!     let chosen = root.chosen();
//!     if let Some(bootargs) = chosen.bootargs() {
//!         println!("The bootargs are: {:?}", bootargs);
//!     }
//!
//!     if let Some(stdout) = chosen.stdout_path() {
//!         println!("It would write stdout to: {}", stdout.path());
//!     }
//!
//!     let soc = root.find_node("/soc");
//!     println!("Does it have a `/soc` node? {}", if soc.is_some() { "yes" } else { "no" });
//!     if let Some(soc) = soc {
//!         println!("...and it has the following children:");
//!         for child in soc.children().iter() {
//!             println!("    {}", child.name());
//!         }
//!     }
//! }
//! ```

#![no_std]
#![warn(missing_docs)]

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests;

/// Trait and types for working with `*-cells` values.
pub mod cell_collector;
/// Helper type aliases.
pub mod helpers;
/// Devicetree node abstractions.
pub mod nodes;
mod parsing;
mod pretty_print;
/// Devicetree property abstractions.
pub mod properties;
mod util;

use helpers::FallibleParser;
use nodes::{
    root::{AllCompatibleIter, AllNodesIter, AllNodesWithNameIter, Root},
    Node,
};
use parsing::{
    aligned::AlignedParser, unaligned::UnalignedParser, NoPanic, Panic, ParseError, Parser, ParserWithMode,
    StringsBlock, StructsBlock,
};
// use standard_nodes::{Aliases, Chosen, Cpu, Memory, MemoryRange, MemoryRegion, Root};

mod sealed {
    pub trait Sealed {}
}

/// Possible errors when attempting to create an `Fdt`
#[derive(Debug, Clone, Copy)]
pub enum FdtError {
    /// The FDT had an invalid magic value
    BadMagic,
    /// The given pointer was null
    BadPtr,
    /// The provided slice is smaller than the required size given by the header
    SliceTooSmall,
    /// An error was encountered during parsing
    ParseError(ParseError),
    /// Attempted to resolve the `phandle` value for a node, but was unable to
    /// locate it.
    MissingPHandleNode(u32),
    /// A parent node is required.
    MissingParent,
    /// A required node with the given name wasn't found.
    MissingRequiredNode(&'static str),
    /// A required property with the given name wasn't found.
    MissingRequiredProperty(&'static str),
    /// Property name contained invalid characters.
    InvalidPropertyValue,
    /// Node name contained invalid characters.
    InvalidNodeName,
    /// A `-cells` property value was unable to be collected into the specified
    /// type.
    CollectCellsError,
}

impl From<ParseError> for FdtError {
    fn from(value: ParseError) -> Self {
        Self::ParseError(value)
    }
}

impl core::fmt::Display for FdtError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FdtError::BadMagic => write!(f, "bad FDT magic value"),
            FdtError::BadPtr => write!(f, "an invalid pointer was passed"),
            FdtError::SliceTooSmall => write!(f, "provided slice is too small"),
            FdtError::ParseError(e) => core::fmt::Display::fmt(e, f),
            FdtError::MissingPHandleNode(value) => {
                write!(f, "a node containing the `phandle` property value of `{value}` was not found")
            }
            FdtError::MissingParent => write!(f, "node parent is not present but needed to parse a property"),
            FdtError::MissingRequiredNode(name) => {
                write!(f, "FDT is missing a required node `{}`", name)
            }
            FdtError::MissingRequiredProperty(name) => {
                write!(f, "FDT node is missing a required property `{}`", name)
            }
            FdtError::InvalidPropertyValue => write!(f, "FDT property value is invalid"),
            FdtError::InvalidNodeName => {
                write!(f, "FDT node contained invalid characters or did not match the expected format")
            }
            FdtError::CollectCellsError => {
                write!(f, "overflow occurred while collecting `#<specifier>-cells` size values into the desired type")
            }
        }
    }
}

/// A flattened devicetree located somewhere in memory
///
/// Note on `Debug` impl: by default the `Debug` impl of this struct will not
/// print any useful information, if you would like a best-effort tree print
/// which looks similar to `dtc`'s output, enable the `pretty-printing` feature
#[derive(Clone, Copy)]
pub struct Fdt<'a, P: ParserWithMode<'a>> {
    structs: StructsBlock<'a, P::Granularity>,
    strings: StringsBlock<'a>,
    header: FdtHeader,
}

impl<'a, P: ParserWithMode<'a>> core::fmt::Debug for Fdt<'a, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Fdt").finish_non_exhaustive()
    }
}

impl<'a, P: ParserWithMode<'a>> core::fmt::Display for Fdt<'a, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut parser: (P::Parser, NoPanic) = <_>::new(self.structs.0, self.strings, self.structs);

        let Ok(node) = parser.parse_root() else {
            return Err(core::fmt::Error);
        };

        pretty_print::print_fdt(f, Root { node })
    }
}

/// FDT header.
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct FdtHeader {
    /// FDT header magic
    pub magic: u32,
    /// Total size in bytes of the FDT structure
    pub total_size: u32,
    /// Offset in bytes from the start of the header to the structure block
    pub structs_offset: u32,
    /// Offset in bytes from the start of the header to the strings block
    pub strings_offset: u32,
    /// Offset in bytes from the start of the header to the memory reservation
    /// block
    pub memory_reserve_map_offset: u32,
    /// FDT version
    pub version: u32,
    /// Last compatible FDT version
    pub last_compatible_version: u32,
    /// System boot CPU ID
    pub boot_cpuid: u32,
    /// Length in bytes of the strings block
    pub strings_size: u32,
    /// Length in bytes of the struct block
    pub structs_size: u32,
}

impl FdtHeader {
    fn valid_magic(&self) -> bool {
        self.magic == 0xd00dfeed
    }
}

impl<'a> Fdt<'a, (UnalignedParser<'a>, Panic)> {
    /// Construct a new `Fdt` from a byte buffer
    pub fn new_unaligned(data: &'a [u8]) -> Result<Self, FdtError> {
        let mut parser = UnalignedParser::new(data, StringsBlock(&[]), StructsBlock(&[]));
        let header = parser.parse_header()?;

        let strings_end = (header.strings_offset + header.strings_size) as usize;
        let structs_end = (header.structs_offset + header.structs_size) as usize;
        if data.len() < strings_end || data.len() < structs_end {
            return Err(FdtError::SliceTooSmall);
        }

        let strings = StringsBlock(&data[header.strings_offset as usize..][..header.strings_size as usize]);
        let structs = StructsBlock(&data[header.structs_offset as usize..][..header.structs_size as usize]);

        if !header.valid_magic() {
            return Err(FdtError::BadMagic);
        } else if data.len() < header.total_size as usize {
            return Err(FdtError::SliceTooSmall);
        }

        Ok(Self { header, structs, strings })
    }

    /// # Safety
    /// This function performs a read to verify the magic value. If the pointer
    /// is invalid this can result in undefined behavior.
    pub unsafe fn from_ptr_unaligned(ptr: *const u8) -> Result<Self, FdtError> {
        if ptr.is_null() {
            return Err(FdtError::BadPtr);
        }

        let tmp_header = core::slice::from_raw_parts(ptr, core::mem::size_of::<FdtHeader>());
        let real_size = usize::try_from(
            UnalignedParser::new(tmp_header, StringsBlock(&[]), StructsBlock(&[])).parse_header()?.total_size,
        )
        .map_err(|_| ParseError::NumericConversionError)?;

        Self::new_unaligned(core::slice::from_raw_parts(ptr, real_size))
    }
}

impl<'a> Fdt<'a, (AlignedParser<'a>, Panic)> {
    /// Construct a new `Fdt` from a `u32`-aligned buffer
    pub fn new(data: &'a [u32]) -> Result<Self, FdtError> {
        let mut parser = AlignedParser::new(data, StringsBlock(&[]), StructsBlock(&[]));
        let header = parser.parse_header()?;

        let strings_end = (header.strings_offset + header.strings_size) as usize / 4;
        let structs_end = (header.structs_offset + header.structs_size) as usize / 4;
        if data.len() < strings_end || data.len() < structs_end {
            return Err(FdtError::SliceTooSmall);
        }

        let strings_start = header.strings_offset as usize;
        let strings_end = strings_start + header.strings_size as usize;
        let strings = StringsBlock(
            util::cast_slice(data)
                .get(strings_start..strings_end)
                .ok_or(FdtError::ParseError(ParseError::UnexpectedEndOfData))?,
        );

        let structs_start = header.structs_offset as usize / 4;
        let structs_end = structs_start + (header.structs_size as usize / 4);
        let structs = StructsBlock(
            data.get(structs_start..structs_end).ok_or(FdtError::ParseError(ParseError::UnexpectedEndOfData))?,
        );

        if !header.valid_magic() {
            return Err(FdtError::BadMagic);
        } else if data.len() < (header.total_size / 4) as usize {
            return Err(FdtError::ParseError(ParseError::UnexpectedEndOfData));
        }

        Ok(Self { header, strings, structs })
    }

    /// # Safety
    /// This function performs a read to verify the magic value. If the pointer
    /// is invalid this can result in undefined behavior.
    pub unsafe fn from_ptr(ptr: *const u32) -> Result<Self, FdtError> {
        if ptr.is_null() {
            return Err(FdtError::BadPtr);
        }

        let tmp_header = core::slice::from_raw_parts(ptr, core::mem::size_of::<FdtHeader>());
        let real_size = usize::try_from(
            AlignedParser::new(tmp_header, StringsBlock(&[]), StructsBlock(&[])).parse_header()?.total_size,
        )
        .map_err(|_| ParseError::NumericConversionError)?;

        Self::new(core::slice::from_raw_parts(ptr, real_size))
    }
}

impl<'a> Fdt<'a, (UnalignedParser<'a>, NoPanic)> {
    /// Construct a new `Fdt` from a byte buffer
    pub fn new_unaligned_fallible(data: &'a [u8]) -> Result<Self, FdtError> {
        let Fdt { header, strings, structs } = Fdt::new_unaligned(data)?;
        Ok(Self { header, strings, structs })
    }

    /// # Safety
    /// This function performs a read to verify the magic value. If the pointer
    /// is invalid this can result in undefined behavior.
    pub unsafe fn from_ptr_unaligned_fallible(ptr: *const u8) -> Result<Self, FdtError> {
        let Fdt { header, strings, structs } = Fdt::from_ptr_unaligned(ptr)?;
        Ok(Self { header, strings, structs })
    }
}

impl<'a> Fdt<'a, (AlignedParser<'a>, NoPanic)> {
    /// Construct a new `Fdt` from a `u32`-aligned buffer which won't panic on invalid data
    pub fn new_fallible(data: &'a [u32]) -> Result<Self, FdtError> {
        let Fdt { header, strings, structs } = Fdt::new(data)?;
        Ok(Self { header, strings, structs })
    }

    /// # Safety
    /// This function performs a read to verify the magic value. If the pointer
    /// is invalid this can result in undefined behavior.
    pub unsafe fn from_ptr_fallible(ptr: *const u32) -> Result<Self, FdtError> {
        let Fdt { header, strings, structs } = Fdt::from_ptr(ptr)?;
        Ok(Self { header, strings, structs })
    }
}

impl<'a, P: ParserWithMode<'a>> Fdt<'a, P> {
    #[inline(always)]
    fn fallible_root(&self) -> Result<Root<'a, FallibleParser<'a, P>>, FdtError> {
        let mut parser = FallibleParser::<'a, P>::new(self.structs.0, self.strings, self.structs);
        Ok(Root { node: parser.parse_root()? })
    }

    /// Return the root (`/`) node, which is always available
    pub fn root(&self) -> P::Output<Root<'a, P>> {
        let mut parser = P::new(self.structs.0, self.strings, self.structs);
        P::to_output(parser.parse_root().map(|node| Root { node: node.fallible() }))
    }

    /// Returns an iterator over all of the strings inside of the strings block
    pub fn strings(&self) -> impl Iterator<Item = &'a str> {
        let mut block = self.strings_block();

        core::iter::from_fn(move || {
            if block.is_empty() {
                return None;
            }

            let cstr = core::ffi::CStr::from_bytes_until_nul(block).ok()?;

            block = &block[cstr.to_bytes().len() + 1..];

            cstr.to_str().ok()
        })
    }

    /// Convenience wrapper around [`Root::find_all_nodes_with_name`]. Returns
    /// an iterator that yields every node with the name that matches `name` in
    /// depth-first order.
    #[track_caller]
    pub fn find_all_nodes_with_name<'b>(&self, name: &'b str) -> P::Output<AllNodesWithNameIter<'a, 'b, P>> {
        P::to_output(self.fallible_root().and_then(|root| {
            root.find_all_nodes_with_name(name).map(|i| AllNodesWithNameIter { iter: i.iter, name: i.name })
        }))
    }

    /// Convenience wrapper around [`Root::find_node_by_name`]. Attempt to find
    /// a node with the given name, returning the first node with a name that
    /// matches `name` in depth-first order.
    #[track_caller]
    pub fn find_node_by_name(&self, name: &str) -> P::Output<Option<Node<'a, P>>> {
        P::to_output(self.fallible_root().and_then(|root| Ok(root.find_node_by_name(name)?.map(|n| n.alt()))))
    }

    /// Convenience wrapper around [`Root::find_node`]. Attempt to find a node
    /// with the given path (with an optional unit address, defaulting to the
    /// first matching name if omitted). If you only have the node name but not
    /// the path, use [`Root::find_node_by_name`] instead.
    #[track_caller]
    pub fn find_node(&self, path: &str) -> P::Output<Option<Node<'a, P>>> {
        P::to_output(self.fallible_root().and_then(|root| Ok(root.find_node(path)?.map(|n| n.alt()))))
    }

    /// Convenience wrapper around [`Root::all_compatible`]. Returns an iterator over
    /// every node within the devicetree which is compatible with at least one
    /// of the compatible strings contained within `with`.
    #[track_caller]
    pub fn all_compatible<'b>(&self, with: &'b [&str]) -> P::Output<AllCompatibleIter<'a, 'b, P>> {
        P::to_output(
            self.fallible_root()
                .and_then(|root| root.all_compatible(with).map(|i| AllCompatibleIter { iter: i.iter, with: i.with })),
        )
    }

    /// Convenience wrapper around [`Root::all_nodes`]. Returns an iterator over
    /// each node in the tree, depth-first, along with its depth in the tree.
    #[track_caller]
    pub fn all_nodes(&self) -> P::Output<AllNodesIter<'a, P>> {
        P::to_output(self.fallible_root().and_then(|root| {
            root.all_nodes().map(|i| AllNodesIter {
                parser: P::new(i.parser.data(), i.parser.strings(), i.parser.structs()),
                parent_index: i.parent_index,
                parents: i.parents,
            })
        }))
    }

    /// Total size of the devicetree in bytes
    pub fn total_size(&self) -> usize {
        self.header.total_size as usize
    }

    /// Header describing this devicetree.
    pub fn header(&self) -> &FdtHeader {
        &self.header
    }

    /// Slice pointing to the raw strings block.
    pub fn strings_block(&self) -> &'a [u8] {
        self.strings.0
    }

    /// Slice pointing to the raw structs block.
    pub fn structs_block(&self) -> &'a [P::Granularity] {
        self.structs.0
    }
}
