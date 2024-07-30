// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod aligned;
pub mod unaligned;

use crate::{
    nodes::{Node, RawNode},
    FdtError, FdtHeader,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct BigEndianU32(u32);

impl BigEndianU32 {
    pub const fn from_ne(n: u32) -> Self {
        Self(n.to_be())
    }

    pub const fn from_le(n: u32) -> Self {
        Self(u32::from_le(n).to_be())
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct BigEndianToken(pub(crate) BigEndianU32);

impl BigEndianToken {
    pub const BEGIN_NODE: Self = Self(BigEndianU32::from_ne(1));
    pub const END_NODE: Self = Self(BigEndianU32::from_ne(2));
    pub const PROP: Self = Self(BigEndianU32::from_ne(3));
    pub const NOP: Self = Self(BigEndianU32::from_ne(4));
    pub const END: Self = Self(BigEndianU32::from_ne(9));
}

pub(crate) struct Stream<'a, T: Copy>(&'a [T]);

impl<'a, T: Copy> Stream<'a, T> {
    #[inline(always)]
    pub(crate) fn new(data: &'a [T]) -> Self {
        Self(data)
    }

    #[inline(always)]
    pub(crate) fn advance(&mut self) -> Option<T> {
        let ret = *self.0.first()?;
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
            Self::InvalidPropertyValue => write!(f, "invalid property value"),
            Self::InvalidTokenValue => {
                write!(f, "encountered invalid FDT token value while parsing")
            }
            Self::NumericConversionError => {
                write!(f, "u32 value too large for usize (this should only occur on 16-bit platforms)")
            }
            Self::UnexpectedEndOfData => {
                write!(f, "encountered end of data while parsing but expected more")
            }
            Self::UnexpectedToken => {
                write!(f, "encountered an unexpected FDT token value while parsing")
            }
        }
    }
}

pub trait PanicMode: crate::sealed::Sealed {
    type Output<T>;
    fn to_output<T>(result: Result<T, FdtError>) -> Self::Output<T>;
}

#[derive(Clone, Copy, Default)]
pub struct NoPanic;

impl crate::sealed::Sealed for NoPanic {}
impl PanicMode for NoPanic {
    type Output<T> = Result<T, FdtError>;

    #[inline(always)]
    fn to_output<T>(result: Result<T, FdtError>) -> Self::Output<T> {
        result
    }
}

#[derive(Clone, Copy, Default)]
pub struct Panic;

impl crate::sealed::Sealed for Panic {}
impl PanicMode for Panic {
    type Output<T> = T;

    #[track_caller]
    #[inline(always)]
    fn to_output<T>(result: Result<T, FdtError>) -> Self::Output<T> {
        result.unwrap()
    }
}

pub trait ParserWithMode<'a>: Parser<'a> + PanicMode + crate::sealed::Sealed {
    type Parser: Parser<'a, Granularity = Self::Granularity>;
    type Mode: PanicMode + Clone + Default;

    fn into_parts(self) -> (<Self as ParserWithMode<'a>>::Parser, <Self as ParserWithMode<'a>>::Mode);
}

impl<'a, T: Parser<'a>, U: PanicMode> crate::sealed::Sealed for (T, U) {}

impl<'a, T: Parser<'a>, U: PanicMode + Clone + Default> Parser<'a> for (T, U) {
    type Granularity = T::Granularity;

    fn new(
        data: &'a [Self::Granularity],
        strings: StringsBlock<'a>,
        structs: StructsBlock<'a, Self::Granularity>,
    ) -> Self {
        (T::new(data, strings, structs), U::default())
    }

    fn data(&self) -> &'a [Self::Granularity] {
        self.0.data()
    }

    fn byte_data(&self) -> &'a [u8] {
        self.0.byte_data()
    }

    fn strings(&self) -> StringsBlock<'a> {
        self.0.strings()
    }

    fn structs(&self) -> StructsBlock<'a, Self::Granularity> {
        self.0.structs()
    }

    fn advance_token(&mut self) -> Result<BigEndianToken, FdtError> {
        self.0.advance_token()
    }

    fn advance_u32(&mut self) -> Result<BigEndianU32, FdtError> {
        self.0.advance_u32()
    }

    fn advance_cstr(&mut self) -> Result<&'a core::ffi::CStr, FdtError> {
        self.0.advance_cstr()
    }

    fn advance_aligned(&mut self, n: usize) {
        self.0.advance_aligned(n)
    }
}

impl<'a, P: Parser<'a>, U: PanicMode> PanicMode for (P, U) {
    type Output<T> = U::Output<T>;

    #[track_caller]
    fn to_output<T>(result: Result<T, FdtError>) -> Self::Output<T> {
        U::to_output(result)
    }
}

impl<'a, T: Parser<'a>, U: PanicMode + Clone + Default + 'static> ParserWithMode<'a> for (T, U) {
    type Mode = U;
    type Parser = T;

    fn into_parts(self) -> (<Self as ParserWithMode<'a>>::Parser, <Self as ParserWithMode<'a>>::Mode) {
        self
    }
}

pub trait Parser<'a>: crate::sealed::Sealed + Clone {
    type Granularity: Copy + core::fmt::Debug;

    fn new(
        data: &'a [Self::Granularity],
        strings: StringsBlock<'a>,
        structs: StructsBlock<'a, Self::Granularity>,
    ) -> Self;
    fn data(&self) -> &'a [Self::Granularity];
    fn byte_data(&self) -> &'a [u8];
    fn strings(&self) -> StringsBlock<'a>;
    fn structs(&self) -> StructsBlock<'a, Self::Granularity>;

    fn advance_token(&mut self) -> Result<BigEndianToken, FdtError>;
    fn peek_token(&mut self) -> Result<BigEndianToken, FdtError> {
        self.clone().advance_token()
    }

    fn advance_u32(&mut self) -> Result<BigEndianU32, FdtError>;
    fn advance_cstr(&mut self) -> Result<&'a core::ffi::CStr, FdtError>;
    fn advance_aligned(&mut self, n: usize);

    fn peek_u32(&self) -> Result<BigEndianU32, FdtError> {
        self.clone().advance_u32()
    }

    fn parse_header(&mut self) -> Result<FdtHeader, FdtError> {
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

    fn parse_root(&mut self) -> Result<Node<'a, Self>, FdtError>
    where
        Self: ParserWithMode<'a>,
    {
        match self.advance_token()? {
            BigEndianToken::BEGIN_NODE => {}
            _ => return Err(FdtError::ParseError(ParseError::UnexpectedToken)),
        }

        let starting_data = self.data();

        match core::mem::size_of::<Self::Granularity>() {
            1 => {
                let byte_data = self.byte_data();
                match byte_data.get(byte_data.len() - 4..).map(<[u8; 4]>::try_from) {
                    Some(Ok(data @ [_, _, _, _])) => match BigEndianToken(BigEndianU32(u32::from_ne_bytes(data))) {
                        BigEndianToken::END => {}
                        _ => return Err(FdtError::ParseError(ParseError::UnexpectedToken)),
                    },
                    _ => return Err(FdtError::ParseError(ParseError::UnexpectedEndOfData)),
                }

                Ok(Node {
                    this: RawNode::new(&starting_data[..starting_data.len() - 4]),
                    parent: None,
                    strings: self.strings(),
                    structs: self.structs(),
                    _mode: core::marker::PhantomData,
                })
            }
            4 => {
                let byte_data = self.byte_data();
                match byte_data.get(byte_data.len() - 4..).map(<[u8; 4]>::try_from) {
                    Some(Ok(data @ [_, _, _, _])) => match BigEndianToken(BigEndianU32(u32::from_ne_bytes(data))) {
                        BigEndianToken::END => {}
                        _ => return Err(FdtError::ParseError(ParseError::UnexpectedToken)),
                    },
                    _ => return Err(FdtError::ParseError(ParseError::UnexpectedEndOfData)),
                }

                Ok(Node {
                    this: RawNode::new(&starting_data[..starting_data.len() - 1]),
                    parent: None,
                    strings: self.strings(),
                    structs: self.structs(),
                    _mode: core::marker::PhantomData,
                })
            }
            _ => unreachable!(),
        }
    }

    fn parse_node(&mut self, parent: Option<&'a RawNode<Self::Granularity>>) -> Result<Node<'a, Self>, FdtError>
    where
        Self: ParserWithMode<'a>,
    {
        match self.advance_token()? {
            BigEndianToken::BEGIN_NODE => {}
            _ => return Err(FdtError::ParseError(ParseError::UnexpectedToken)),
        }

        let starting_data = self.data();
        let starting_len = starting_data.len();

        self.advance_cstr()?;

        while self.peek_token()? == BigEndianToken::PROP {
            self.parse_raw_property()?;
        }

        let mut depth = 0;
        loop {
            let token = self.peek_token()?;
            match token {
                BigEndianToken::BEGIN_NODE => depth += 1,
                BigEndianToken::END_NODE => match depth {
                    0 => break,
                    _ => {
                        depth -= 1;
                        continue;
                    }
                },
                _ => return Err(FdtError::ParseError(ParseError::InvalidTokenValue)),
            }

            let _ = self.advance_token();

            self.advance_cstr()?;

            while self.peek_token()? == BigEndianToken::PROP {
                self.parse_raw_property()?;
            }
        }

        let ending_len = self.data().len();

        match self.advance_token()? {
            BigEndianToken::END_NODE => Ok(Node {
                this: RawNode::new(
                    starting_data.get(..starting_len - ending_len).ok_or(ParseError::UnexpectedEndOfData)?,
                ),
                parent,
                strings: self.strings(),
                structs: self.structs(),
                _mode: core::marker::PhantomData,
            }),
            _ => Err(FdtError::ParseError(ParseError::UnexpectedToken)),
        }
    }

    fn parse_raw_property(&mut self) -> Result<(usize, &'a [u8]), FdtError> {
        match self.advance_token()? {
            BigEndianToken::PROP => {
                // Properties are in the format: <data len> <name offset> <data...>
                let len =
                    usize::try_from(self.advance_u32()?.to_ne()).map_err(|_| ParseError::NumericConversionError)?;
                let name_offset =
                    usize::try_from(self.advance_u32()?.to_ne()).map_err(|_| ParseError::NumericConversionError)?;
                let data = self.byte_data().get(..len).ok_or(ParseError::UnexpectedEndOfData)?;

                self.advance_aligned(data.len());

                Ok((name_offset, data))
            }
            _ => Err(FdtError::ParseError(ParseError::UnexpectedToken)),
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct StringsBlock<'a>(pub(crate) &'a [u8]);

impl<'a> StringsBlock<'a> {
    pub fn offset_at(self, offset: usize) -> Result<&'a str, FdtError> {
        core::ffi::CStr::from_bytes_until_nul(self.0.get(offset..).ok_or(ParseError::UnexpectedEndOfData)?)
            .map_err(|_| ParseError::InvalidCStrValue)?
            .to_str()
            .map_err(|_| ParseError::InvalidCStrValue.into())
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct StructsBlock<'a, G>(pub(crate) &'a [G]);
