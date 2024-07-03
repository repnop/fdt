// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

//! # `fdt`
//!
//! A pure-Rust `#![no_std]` crate for parsing Flattened Devicetrees, with the goal of having a
//! very ergonomic and idiomatic API.
//!
//! [![crates.io](https://img.shields.io/crates/v/fdt.svg)](https://crates.io/crates/fdt) [![Documentation](https://docs.rs/fdt/badge.svg)](https://docs.rs/fdt) ![Build](https://github.com/repnop/fdt/actions/workflows/test.yml/badge.svg?branch=master&event=push)
//!
//! ## License
//!
//! This crate is licensed under the Mozilla Public License 2.0 (see the LICENSE file).
//!
//! ## Example
//!
//! ```rust,no_run
//! static MY_FDT: &[u8] = include_bytes!("../dtb/test.dtb");
//!
//! fn main() {
//!     let fdt = fdt::Fdt::new(MY_FDT).unwrap();
//!
//!     println!("This is a devicetree representation of a {}", fdt.root().model());
//!     println!("...which is compatible with at least: {}", fdt.root().compatible().first());
//!     println!("...and has {} CPU(s)", fdt.cpus().count());
//!     println!(
//!         "...and has at least one memory location at: {:#X}\n",
//!         fdt.memory().regions().next().unwrap().starting_address as usize
//!     );
//!
//!     let chosen = fdt.chosen();
//!     if let Some(bootargs) = chosen.bootargs() {
//!         println!("The bootargs are: {:?}", bootargs);
//!     }
//!
//!     if let Some(stdout) = chosen.stdout() {
//!         println!("It would write stdout to: {}", stdout.name());
//!     }
//!
//!     let soc = fdt.find_node("/soc");
//!     println!("Does it have a `/soc` node? {}", if soc.is_some() { "yes" } else { "no" });
//!     if let Some(soc) = soc {
//!         println!("...and it has the following children:");
//!         for child in soc.children() {
//!             println!("    {}", child.name);
//!         }
//!     }
//! }
//! ```

#![no_std]

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests;

mod nodes;
mod parsing;
mod pretty_print;
pub mod properties;
pub mod standard_nodes;
mod util;

use parsing::{
    aligned::AlignedParser, unaligned::UnalignedParser, NoPanic, Panic, ParseError, Parser,
    ParserWithMode, StringsBlock, StructsBlock,
};
use standard_nodes::Root;
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
    /// An error was encountered during parsing
    ParseError(ParseError),
    PHandleNotFound(u32),
    MissingRequiredNode(&'static str),
    MissingRequiredProperty(&'static str),
    InvalidPropertyValue,
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
            FdtError::ParseError(e) => core::fmt::Display::fmt(e, f),
            FdtError::PHandleNotFound(value) => write!(
                f,
                "a node containing the `phandle` property value of `{value}` was not found"
            ),
            FdtError::MissingRequiredNode(name) => {
                write!(f, "FDT is missing a required node `{}`", name)
            }
            FdtError::MissingRequiredProperty(name) => {
                write!(f, "FDT node is missing a required property `{}`", name)
            }
            FdtError::InvalidPropertyValue => write!(f, "FDT property value is invalid"),
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
    parser: P,
    header: FdtHeader,
    _lifetime: core::marker::PhantomData<&'a [u8]>,
}

impl<'a, P: ParserWithMode<'a>> core::fmt::Debug for Fdt<'a, P> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Fdt").finish_non_exhaustive()
    }
}

// impl<'a, P: Parser<'a>> core::fmt::Display for Fdt<'a, P> {
//     fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//         pretty_print::print_node(f, self.root().node, 0)
//     }
// }

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
        let strings =
            StringsBlock(&data[header.strings_offset as usize..][..header.strings_size as usize]);
        let structs =
            StructsBlock(&data[header.structs_offset as usize..][..header.structs_size as usize]);

        if !header.valid_magic() {
            return Err(FdtError::BadMagic);
        } else if data.len() < header.total_size as usize {
            return Err(FdtError::ParseError(ParseError::UnexpectedEndOfData));
        }

        Ok(Self {
            header,
            parser: (UnalignedParser::new(structs.0, strings, structs), Panic),
            _lifetime: core::marker::PhantomData,
        })
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
            UnalignedParser::new(tmp_header, StringsBlock(&[]), StructsBlock(&[]))
                .parse_header()?
                .total_size,
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
            data.get(structs_start..structs_end)
                .ok_or(FdtError::ParseError(ParseError::UnexpectedEndOfData))?,
        );

        if !header.valid_magic() {
            return Err(FdtError::BadMagic);
        } else if data.len() < (header.total_size / 4) as usize {
            return Err(FdtError::ParseError(ParseError::UnexpectedEndOfData));
        }

        Ok(Self {
            header,
            parser: (AlignedParser::new(structs.0, strings, structs), Panic),
            _lifetime: core::marker::PhantomData,
        })
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
            AlignedParser::new(tmp_header, StringsBlock(&[]), StructsBlock(&[]))
                .parse_header()?
                .total_size,
        )
        .map_err(|_| ParseError::NumericConversionError)?;

        Self::new(core::slice::from_raw_parts(ptr, real_size))
    }
}

impl<'a> Fdt<'a, (UnalignedParser<'a>, NoPanic)> {
    /// Construct a new `Fdt` from a byte buffer
    pub fn new_unaligned_fallible(data: &'a [u8]) -> Result<Self, FdtError> {
        let Fdt { parser, header, .. } = Fdt::new_unaligned(data)?;
        Ok(Self {
            parser: (
                UnalignedParser::new(parser.data(), parser.strings(), parser.structs()),
                NoPanic,
            ),
            header,
            _lifetime: core::marker::PhantomData,
        })
    }

    /// # Safety
    /// This function performs a read to verify the magic value. If the pointer
    /// is invalid this can result in undefined behavior.
    pub unsafe fn from_ptr_unaligned_fallible(ptr: *const u8) -> Result<Self, FdtError> {
        let Fdt { parser, header, .. } = Fdt::from_ptr_unaligned(ptr)?;
        Ok(Self {
            parser: (
                UnalignedParser::new(parser.data(), parser.strings(), parser.structs()),
                NoPanic,
            ),
            header,
            _lifetime: core::marker::PhantomData,
        })
    }
}

impl<'a> Fdt<'a, (AlignedParser<'a>, NoPanic)> {
    /// Construct a new `Fdt` from a `u32`-aligned buffer which won't panic on invalid data
    pub fn new_fallible(data: &'a [u32]) -> Result<Self, FdtError> {
        let Fdt { parser, header, .. } = Fdt::new(data)?;
        Ok(Self {
            parser: (
                AlignedParser::new(parser.data(), parser.strings(), parser.structs()),
                NoPanic,
            ),
            header,
            _lifetime: core::marker::PhantomData,
        })
    }

    /// # Safety
    /// This function performs a read to verify the magic value. If the pointer
    /// is invalid this can result in undefined behavior.
    pub unsafe fn from_ptr_fallible(ptr: *const u32) -> Result<Self, FdtError> {
        let Fdt { parser, header, .. } = Fdt::from_ptr(ptr)?;
        Ok(Self {
            parser: (
                AlignedParser::new(parser.data(), parser.strings(), parser.structs()),
                NoPanic,
            ),
            header,
            _lifetime: core::marker::PhantomData,
        })
    }
}

impl<'a, P: ParserWithMode<'a>> Fdt<'a, P> {
    /// Return the `/aliases` node, if one exists
    // pub fn aliases(&self) -> Option<Aliases<'_, 'a>> {
    //     Some(Aliases {
    //         node: node::find_node(&mut FdtData::new(self.structs_block()), "/aliases", self, None)?,
    //         header: self,
    //     })
    // }

    /// Searches for the `/chosen` node, which is always available
    // pub fn chosen(&self) -> Chosen<'_, 'a> {
    //     node::find_node(&mut FdtData::new(self.structs_block()), "/chosen", self, None)
    //         .map(|node| Chosen { node })
    //         .expect("/chosen is required")
    // }

    /// Return the `/cpus` node, which is always available
    // pub fn cpus(&self) -> impl Iterator<Item = Cpu<'_, 'a>> {
    //     let parent = self.find_node("/cpus").expect("/cpus is a required node");

    //     parent
    //         .children()
    //         .filter(|c| c.name.split('@').next().unwrap() == "cpu")
    //         .map(move |cpu| Cpu { parent, node: cpu })
    // }

    /// Returns the memory node, which is always available
    // pub fn memory(&self) -> Memory<'_, 'a> {
    //     Memory { node: self.find_node("/memory").expect("requires memory node") }
    // }

    /// Returns an iterator over the memory reservations
    // pub fn memory_reservations(&self) -> impl Iterator<Item = MemoryReservation> + 'a {
    //     let mut stream = FdtData::new(&self.data[self.header.off_mem_rsvmap.to_ne() as usize..]);
    //     let mut done = false;

    //     core::iter::from_fn(move || {
    //         if stream.is_empty() || done {
    //             return None;
    //         }

    //         let res = MemoryReservation::from_bytes(&mut stream)?;

    //         if res.address() as usize == 0 && res.size() == 0 {
    //             done = true;
    //             return None;
    //         }

    //         Some(res)
    //     })
    // }

    /// Return reference to raw data. This can be used to obtain the original pointer passed to
    /// [Fdt::from_ptr].
    ///
    /// # Example
    /// ```
    /// # let fdt_ref: &[u8] = include_bytes!("../dtb/test.dtb");
    /// # let original_pointer = fdt_ref.as_ptr();
    /// let fdt = unsafe{fdt::Fdt::from_ptr(original_pointer)}.unwrap();
    /// assert_eq!(fdt.raw_data().as_ptr(), original_pointer);
    /// ```
    // pub fn raw_data(&self) -> &'a [P::Granularity] {
    //     // self.structs
    // }

    /// Return the root (`/`) node, which is always available
    pub fn root(&self) -> P::Output<Root<'a, P>> {
        let mut parser = self.parser.clone();
        P::to_output(parser.parse_root().map(|node| Root { node }))
    }

    /// Returns the first node that matches the node path, if you want all that
    /// match the path, use `find_all_nodes`. This will automatically attempt to
    /// resolve aliases if `path` is not found.
    ///
    /// Node paths must begin with a leading `/` and are ASCII only. Passing in
    /// an invalid node path or non-ASCII node name in the path will return
    /// `None`, as they will not be found within the devicetree structure.
    ///
    /// Note: if the address of a node name is left out, the search will find
    /// the first node that has a matching name, ignoring the address portion if
    /// it exists.
    // pub fn find_node(&self, path: &str) -> Option<node::FdtNode<'_, 'a>> {
    //     let node = node::find_node(&mut FdtData::new(self.structs_block()), path, self, None);
    //     node.or_else(|| self.aliases()?.resolve_node(path))
    // }

    /// Searches for a node which contains a `compatible` property and contains
    /// one of the strings inside of `with`
    // pub fn find_compatible(&self, with: &[&str]) -> Option<node::FdtNode<'_, 'a>> {
    //     self.all_nodes().find(|n| {
    //         n.compatible().and_then(|compats| compats.all().find(|c| with.contains(c))).is_some()
    //     })
    // }

    /// Searches for the given `phandle`
    // pub fn find_phandle(&self, phandle: u32) -> Option<node::FdtNode<'_, 'a>> {
    //     self.all_nodes().find(|n| {
    //         n.properties()
    //             .find(|p| p.name == "phandle")
    //             .and_then(|p| Some(BigEndianU32::from_bytes(p.value)?.to_ne() == phandle))
    //             .unwrap_or(false)
    //     })
    // }

    /// Returns an iterator over all of the available nodes with the given path.
    /// This does **not** attempt to find any node with the same name as the
    /// provided path, if you're looking to do that, [`Fdt::all_nodes`] will
    /// allow you to iterate over each node's name and filter for the desired
    /// node(s).
    ///
    /// For example:
    /// ```rust
    /// static MY_FDT: &[u8] = include_bytes!("../dtb/test.dtb");
    ///
    /// let fdt = fdt::Fdt::new(MY_FDT).unwrap();
    ///
    /// for node in fdt.find_all_nodes("/soc/virtio_mmio") {
    ///     println!("{}", node.name);
    /// }
    /// ```
    /// prints:
    /// ```notrust
    /// virtio_mmio@10008000
    /// virtio_mmio@10007000
    /// virtio_mmio@10006000
    /// virtio_mmio@10005000
    /// virtio_mmio@10004000
    /// virtio_mmio@10003000
    /// virtio_mmio@10002000
    /// virtio_mmio@10001000
    /// ```
    // pub fn find_all_nodes(&self, path: &'a str) -> impl Iterator<Item = node::FdtNode<'_, 'a>> {
    //     let mut done = false;
    //     let only_root = path == "/";
    //     let valid_path = path.chars().fold(0, |acc, c| acc + if c == '/' { 1 } else { 0 }) >= 1;

    //     let mut path_split = path.rsplitn(2, '/');
    //     let child_name = path_split.next().unwrap();
    //     let parent = match path_split.next() {
    //         Some("") => Some(self.root().node),
    //         Some(s) => node::find_node(&mut FdtData::new(self.structs_block()), s, self, None),
    //         None => None,
    //     };

    //     let (parent, bad_parent) = match parent {
    //         Some(parent) => (parent, false),
    //         None => (self.find_node("/").unwrap(), true),
    //     };

    //     let mut child_iter = parent.children();

    //     core::iter::from_fn(move || {
    //         if done || !valid_path || bad_parent {
    //             return None;
    //         }

    //         if only_root {
    //             done = true;
    //             return self.find_node("/");
    //         }

    //         let mut ret = None;

    //         #[allow(clippy::while_let_on_iterator)]
    //         while let Some(child) = child_iter.next() {
    //             if child.name.split('@').next()? == child_name {
    //                 ret = Some(child);
    //                 break;
    //             }
    //         }

    //         ret
    //     })
    // }

    /// Returns an iterator over all of the nodes in the devicetree, depth-first
    // pub fn all_nodes(&self) -> impl Iterator<Item = node::FdtNode<'_, 'a>> {
    //     node::all_nodes(self)
    // }

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

    /// Total size of the devicetree in bytes
    pub fn total_size(&self) -> usize {
        self.header.total_size as usize
    }

    pub fn header(&self) -> &FdtHeader {
        &self.header
    }

    pub fn strings_block(&self) -> &'a [u8] {
        self.parser.strings().0
    }

    pub fn structs_block(&self) -> &'a [P::Granularity] {
        self.parser.structs().0
    }
}
