// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::FdtError;

use super::{
    BigEndianToken, BigEndianU32, BigEndianU64, ParseError, Parser, Stream, StringsBlock,
    StructsBlock,
};

pub struct UnalignedParser<'a> {
    stream: Stream<'a, u8>,
    strings: StringsBlock<'a>,
    structs: StructsBlock<'a, u8>,
}

impl Clone for UnalignedParser<'_> {
    fn clone(&self) -> Self {
        Self { stream: self.stream.clone(), strings: self.strings, structs: self.structs }
    }
}

impl crate::sealed::Sealed for UnalignedParser<'_> {}
impl<'a> Parser<'a> for UnalignedParser<'a> {
    type Granularity = u8;

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
        self.stream.0
    }

    fn strings(&self) -> super::StringsBlock<'a> {
        self.strings
    }

    fn structs(&self) -> StructsBlock<'a, Self::Granularity> {
        self.structs
    }

    fn advance_token(&mut self) -> Result<BigEndianToken, FdtError> {
        loop {
            match BigEndianToken(self.advance_u32()?) {
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
        if self.stream.0.len() < core::mem::size_of::<u32>() {
            return Err(FdtError::ParseError(ParseError::UnexpectedEndOfData));
        }

        let data = self.stream.0;
        self.stream.skip_many(4);

        // SAFETY: The buffer has at least 4 bytes available to read
        Ok(BigEndianU32::from_be(unsafe { core::ptr::read_unaligned(data.as_ptr().cast::<u32>()) }))
    }

    fn advance_u64(&mut self) -> Result<BigEndianU64, FdtError> {
        if self.stream.0.len() < core::mem::size_of::<u64>() {
            return Err(FdtError::ParseError(ParseError::UnexpectedEndOfData));
        }

        let data = self.stream.0;
        self.stream.skip_many(4);

        // SAFETY: The buffer has at least 4 bytes available to read
        Ok(BigEndianU64::from_be(unsafe { core::ptr::read_unaligned(data.as_ptr().cast::<u64>()) }))
    }

    fn advance_cstr(&mut self) -> Result<&'a core::ffi::CStr, FdtError> {
        let cstr = core::ffi::CStr::from_bytes_until_nul(self.stream.0)
            .map_err(|_| ParseError::InvalidCStrValue)?;

        // Round up to the next multiple of 4, if necessary
        let skip = (cstr.to_bytes_with_nul().len() + 3) & !3;
        self.stream.skip_many(skip);

        Ok(cstr)
    }

    fn advance_aligned(&mut self, n: usize) {
        // Round up to the next multiple of 4, if necessary
        let skip = (n + 3) & !3;
        self.stream.skip_many(skip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_u32() {
        let n = BigEndianU32::from_ne(0xF00DCAFE);
        let mut parser = UnalignedParser::new(
            unsafe { core::slice::from_raw_parts(&n as *const BigEndianU32 as *const u8, 4) },
            StringsBlock(&[]),
            StructsBlock(&[]),
        );
        let m = parser.advance_u32().unwrap();

        assert_eq!(n, m);
    }

    #[test]
    fn advance_u64() {
        let n = BigEndianU64::from_ne(0xF00DCAFEDEADFEED);
        let mut parser = UnalignedParser::new(
            unsafe { core::slice::from_raw_parts(&n as *const BigEndianU64 as *const u8, 8) },
            StringsBlock(&[]),
            StructsBlock(&[]),
        );
        let m = parser.advance_u64().unwrap();

        assert_eq!(n, m);
    }
}
