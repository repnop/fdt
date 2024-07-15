// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::FdtError;

use super::{BigEndianToken, BigEndianU32, ParseError, Parser, Stream, StringsBlock, StructsBlock};

pub struct AlignedParser<'a> {
    stream: Stream<'a, u32>,
    strings: StringsBlock<'a>,
    structs: StructsBlock<'a, u32>,
}

impl Clone for AlignedParser<'_> {
    fn clone(&self) -> Self {
        Self { stream: self.stream.clone(), strings: self.strings, structs: self.structs }
    }
}

impl crate::sealed::Sealed for AlignedParser<'_> {}
impl<'a> Parser<'a> for AlignedParser<'a> {
    type Granularity = u32;

    fn new(
        data: &'a [Self::Granularity],
        strings: StringsBlock<'a>,
        structs: StructsBlock<'a, Self::Granularity>,
    ) -> Self {
        Self { stream: Stream::new(data), strings, structs }
    }

    fn data(&self) -> &'a [Self::Granularity] {
        self.stream.0
    }

    fn byte_data(&self) -> &'a [u8] {
        // SAFETY: it is always valid to cast a `u32` to 4 `u8`s
        unsafe {
            core::slice::from_raw_parts(
                self.stream.0.as_ptr().cast::<u8>(),
                self.stream.0.len() * 4,
            )
        }
    }

    fn strings(&self) -> super::StringsBlock<'a> {
        self.strings
    }

    fn structs(&self) -> StructsBlock<'a, Self::Granularity> {
        self.structs
    }

    fn advance_token(&mut self) -> Result<BigEndianToken, FdtError> {
        loop {
            match BigEndianToken(
                self.stream.advance().map(BigEndianU32).ok_or(ParseError::UnexpectedEndOfData)?,
            ) {
                BigEndianToken::NOP => continue,
                token @ BigEndianToken::BEGIN_NODE
                | token @ BigEndianToken::END_NODE
                | token @ BigEndianToken::PROP
                | token @ BigEndianToken::END => break Ok(token),
                _ => break Err(FdtError::ParseError(ParseError::InvalidTokenValue)),
            }
        }
    }

    fn advance_u32(&mut self) -> Result<BigEndianU32, FdtError> {
        self.stream
            .advance()
            .map(BigEndianU32)
            .ok_or(FdtError::ParseError(ParseError::UnexpectedEndOfData))
    }

    fn advance_cstr(&mut self) -> Result<&'a core::ffi::CStr, FdtError> {
        // SAFETY: It is safe to reinterpret the stream data to a smaller integer size
        let bytes = unsafe {
            core::slice::from_raw_parts(
                self.stream.0.as_ptr().cast::<u8>(),
                self.stream.0.len() * 4,
            )
        };
        let cstr = core::ffi::CStr::from_bytes_until_nul(bytes)
            .map_err(|_| ParseError::InvalidCStrValue)?;

        // Round up to the next multiple of 4, if necessary
        let skip = ((cstr.to_bytes_with_nul().len() + 3) & !3) / 4;
        self.stream.skip_many(skip);

        Ok(cstr)
    }

    fn advance_aligned(&mut self, n: usize) {
        // Round up to the next multiple of 4, if necessary
        let skip = ((n + 3) & !3) / 4;
        self.stream.skip_many(skip);
    }
}
