// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod aligned;
pub mod unaligned;

use self::{aligned::AlignedParser, unaligned::UnalignedParser};
use crate::{
    nodes::{Node, RawNode},
    FdtHeader,
};

#[derive(Debug, Clone, Copy)]
pub struct FdtData<'a> {
    bytes: &'a [u8],
}

impl<'a> FdtData<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }

    pub fn u32(&mut self) -> Option<BigEndianU32> {
        let ret = BigEndianU32::from_bytes(self.bytes)?;
        self.skip(4);

        Some(ret)
    }

    pub fn u64(&mut self) -> Option<BigEndianU64> {
        let ret = BigEndianU64::from_bytes(self.bytes)?;
        self.skip(8);

        Some(ret)
    }

    pub fn skip(&mut self, n_bytes: usize) {
        self.bytes = self.bytes.get(n_bytes..).unwrap_or_default()
    }

    pub fn remaining(&self) -> &'a [u8] {
        self.bytes
    }

    pub fn peek_u32(&self) -> Option<BigEndianU32> {
        Self::new(self.remaining()).u32()
    }

    pub fn is_empty(&self) -> bool {
        self.remaining().is_empty()
    }

    pub fn skip_nops(&mut self) {
        while let Some(4) = self.peek_u32().map(|n| n.to_ne()) {
            let _ = self.u32();
        }
    }

    pub fn take(&mut self, bytes: usize) -> Option<&'a [u8]> {
        if self.bytes.len() >= bytes {
            let ret = &self.bytes[..bytes];
            self.skip(bytes);

            return Some(ret);
        }

        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct BigEndianU32(u32);

impl BigEndianU32 {
    pub const fn from_ne(n: u32) -> Self {
        Self(n.to_be())
    }

    pub const fn from_le(n: u32) -> Self {
        Self(u32::from_le(n))
    }

    pub const fn from_be(n: u32) -> Self {
        Self(n)
    }

    pub const fn to_ne(self) -> u32 {
        u32::from_be(self.0)
    }

    pub const fn to_be(self) -> u32 {
        self.0
    }

    pub(crate) fn from_bytes(bytes: &[u8]) -> Option<Self> {
        Some(BigEndianU32(u32::from_ne_bytes(bytes.get(..4)?.try_into().unwrap())))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct BigEndianU64(u64);

impl BigEndianU64 {
    pub const fn from_ne(n: u64) -> Self {
        Self(n.to_be())
    }

    pub const fn from_le(n: u64) -> Self {
        Self(u64::from_le(n))
    }

    pub const fn from_be(n: u64) -> Self {
        Self(n)
    }

    pub const fn to_ne(self) -> u64 {
        u64::from_be(self.0)
    }

    pub const fn to_be(self) -> u64 {
        self.0
    }

    pub(crate) fn from_bytes(bytes: &[u8]) -> Option<Self> {
        Some(BigEndianU64(u64::from_ne_bytes(bytes.get(..8)?.try_into().unwrap())))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct BigEndianToken(BigEndianU32);

impl BigEndianToken {
    pub const BEGIN_NODE: Self = Self(BigEndianU32::from_ne(1));
    pub const END_NODE: Self = Self(BigEndianU32::from_ne(2));
    pub const PROP: Self = Self(BigEndianU32::from_ne(3));
    pub const NOP: Self = Self(BigEndianU32::from_ne(4));
    pub const END: Self = Self(BigEndianU32::from_ne(5));
}

pub(crate) struct Stream<'a, T: Copy>(&'a [T]);

impl<'a, T: Copy> Stream<'a, T> {
    #[inline(always)]
    pub(crate) fn new(data: &'a [T]) -> Self {
        Self(data)
    }

    #[inline(always)]
    pub(crate) fn advance(&mut self) -> Option<T> {
        let ret = self.0[0];
        self.0 = self.0.get(1..)?;
        Some(ret)
    }

    pub(crate) fn skip_many(&mut self, n: usize) {
        self.0 = self.0.get(n..).unwrap_or_default();
    }
}

impl<'a, T: Copy> Clone for Stream<'a, T> {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

mod sealed {
    pub trait Sealed {}
}

#[derive(Debug, Clone, Copy)]
pub enum ParseError {
    NumericConversionError,
    InvalidCStrValue,
    InvalidPropertyValue,
    InvalidTokenValue,
    UnexpectedToken,
    UnexpectedEndOfData,
}

impl core::fmt::Display for ParseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidCStrValue => write!(f, "cstr was either non-terminated or invalid ASCII"),
            Self::InvalidTokenValue => {
                write!(f, "encountered invalid FDT token value while parsing")
            }
            Self::NumericConversionError => write!(
                f,
                "u32 value too large for usize (this should only occur on 16-bit platforms)"
            ),
            Self::UnexpectedEndOfData => {
                write!(f, "encountered end of data while parsing but expected more")
            }
            Self::UnexpectedToken => {
                write!(f, "encountered an unexpected FDT token value while parsing")
            }
        }
    }
}

pub trait ParserForSize: sealed::Sealed {
    type Parser<'a>: Parser<'a, Granularity = Self>;
}

impl sealed::Sealed for u8 {}
impl ParserForSize for u8 {
    type Parser<'a> = UnalignedParser<'a>;
}

impl sealed::Sealed for u32 {}
impl ParserForSize for u32 {
    type Parser<'a> = AlignedParser<'a>;
}

pub trait PanicMode: sealed::Sealed {
    type Output<T>;
    fn to_output<T>(result: Result<T, ParseError>) -> Self::Output<T>;
    fn transpose<T>(result: Self::Output<Option<T>>) -> Option<Self::Output<T>>;
    fn reverse_transpose<T>(result: Option<Self::Output<T>>) -> Self::Output<Option<T>>;
    fn ok_as_ref<T>(output: &Self::Output<T>) -> Option<&T>;
    fn ok<T>(output: Self::Output<T>) -> Option<T>;
    fn to_result<T>(output: Self::Output<T>) -> Result<T, ParseError>;
}

pub struct NoPanic;

impl sealed::Sealed for NoPanic {}
impl PanicMode for NoPanic {
    type Output<T> = Result<T, ParseError>;

    #[inline(always)]
    fn to_output<T>(result: Result<T, ParseError>) -> Self::Output<T> {
        result
    }

    fn transpose<T>(result: Self::Output<Option<T>>) -> Option<Self::Output<T>> {
        result.transpose()
    }

    fn reverse_transpose<T>(result: Option<Self::Output<T>>) -> Self::Output<Option<T>> {
        result.transpose()
    }

    fn ok_as_ref<T>(output: &Self::Output<T>) -> Option<&T> {
        output.ok().as_ref()
    }

    fn ok<T>(output: Self::Output<T>) -> Option<T> {
        output.ok()
    }

    fn to_result<T>(output: Self::Output<T>) -> Result<T, ParseError> {
        output
    }
}

pub struct Panic;

impl sealed::Sealed for Panic {}
impl PanicMode for Panic {
    type Output<T> = T;

    #[track_caller]
    #[inline(always)]
    fn to_output<T>(result: Result<T, ParseError>) -> Self::Output<T> {
        result.unwrap()
    }

    #[track_caller]
    #[inline(always)]
    fn transpose<T>(result: Self::Output<Option<T>>) -> Option<Self::Output<T>> {
        result
    }

    fn reverse_transpose<T>(result: Option<Self::Output<T>>) -> Self::Output<Option<T>> {
        result
    }

    fn ok_as_ref<T>(output: &Self::Output<T>) -> Option<&T> {
        Some(output)
    }

    fn ok<T>(output: Self::Output<T>) -> Option<T> {
        Some(output)
    }

    fn to_result<T>(output: Self::Output<T>) -> Result<T, ParseError> {
        Ok(output)
    }
}

pub trait Parser<'a>: sealed::Sealed + Clone {
    type Granularity: Copy + ParserForSize;

    fn new(data: &'a [Self::Granularity], strings: &'a [u8]) -> Self;
    fn data(&self) -> &'a [Self::Granularity];
    fn byte_data(&self) -> &'a [u8];
    fn strings(&self) -> StringsBlock<'a>;

    fn advance_token(&mut self) -> Result<BigEndianToken, ParseError>;
    fn peek_token(&mut self) -> Result<BigEndianToken, ParseError> {
        self.clone().advance_token()
    }

    fn advance_u32(&mut self) -> Result<BigEndianU32, ParseError>;
    fn advance_u64(&mut self) -> Result<BigEndianU64, ParseError>;
    fn advance_cstr(&mut self) -> Result<&'a core::ffi::CStr, ParseError>;
    fn advance_aligned(&mut self, n: usize);

    fn peek_u32(&self) -> Result<BigEndianU32, ParseError> {
        self.clone().advance_u32()
    }

    fn peek_u64(&self) -> Result<BigEndianU64, ParseError> {
        self.clone().advance_u64()
    }

    fn parse_header(&mut self) -> Result<FdtHeader, ParseError> {
        let magic = self.advance_u32()?.to_ne();
        let total_size = self.advance_u32()?.to_ne();
        let struct_offset = self.advance_u32()?.to_ne();
        let strings_offset = self.advance_u32()?.to_ne();
        let memory_reserve_map_offset = self.advance_u32()?.to_ne();
        let version = self.advance_u32()?.to_ne();
        let last_compatible_version = self.advance_u32()?.to_ne();
        let boot_cpuid = self.advance_u32()?.to_ne();
        let strings_size = self.advance_u32()?.to_ne();
        let structs_size = self.advance_u32()?.to_ne();

        Ok(FdtHeader {
            magic,
            total_size,
            structs_offset: struct_offset,
            strings_offset,
            memory_reserve_map_offset,
            version,
            last_compatible_version,
            boot_cpuid,
            strings_size,
            structs_size,
        })
    }

    fn parse_node<Mode: PanicMode>(
        &mut self,
        parent: Option<&'a RawNode<Self::Granularity>>,
    ) -> Result<Node<'a, Self::Granularity, Mode>, ParseError> {
        let starting_len = self.data().len();
        let starting_data = self.data();

        match self.advance_token()? {
            BigEndianToken::BEGIN_NODE => {}
            _ => return Err(ParseError::UnexpectedToken),
        }

        self.advance_cstr()?;

        while self.peek_token()? == BigEndianToken::PROP {
            self.parse_raw_property()?;
        }

        let mut depth = 0;
        loop {
            let token = self.advance_token()?;
            match token {
                BigEndianToken::BEGIN_NODE => depth += 1,
                BigEndianToken::END_NODE => match depth {
                    0 => break,
                    _ => {
                        depth -= 1;
                        continue;
                    }
                },
            }

            self.advance_cstr()?;

            while self.peek_token()? == BigEndianToken::PROP {
                self.parse_raw_property()?;
            }
        }

        let ending_len = self.data().len();

        match self.advance_token()? {
            BigEndianToken::END_NODE => Ok(Node {
                this: RawNode::new(
                    starting_data
                        .get(..starting_len - ending_len)
                        .ok_or(ParseError::UnexpectedEndOfData)?,
                ),
                parent,
                strings: self.strings(),
                _mode: core::marker::PhantomData,
            }),
            _ => return Err(ParseError::UnexpectedToken),
        }
    }

    fn parse_raw_property(&mut self) -> Result<(usize, &'a [u8]), ParseError> {
        match self.advance_token()? {
            BigEndianToken::PROP => {
                // Properties are in the format: <data len> <name offset> <data...>
                let len = usize::try_from(self.advance_u32()?.to_ne())
                    .map_err(|_| ParseError::NumericConversionError)?;
                let name_offset = usize::try_from(self.advance_u32()?.to_ne())
                    .map_err(|_| ParseError::NumericConversionError)?;
                let data = self.byte_data().get(..len).ok_or(ParseError::UnexpectedEndOfData)?;

                self.advance_aligned(data.len());

                Ok((name_offset, data))
            }
            _ => Err(ParseError::UnexpectedToken),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct StringsBlock<'a>(pub(crate) &'a [u8]);

impl<'a> StringsBlock<'a> {
    pub fn offset_at(self, offset: usize) -> Result<&'a str, ParseError> {
        core::ffi::CStr::from_bytes_until_nul(
            self.0.get(offset..).ok_or(ParseError::UnexpectedEndOfData)?,
        )
        .map_err(|_| ParseError::InvalidCStrValue)?
        .to_str()
        .map_err(|_| ParseError::InvalidCStrValue)
    }
}
