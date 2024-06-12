// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use super::{
    sealed, BigEndianToken, BigEndianU32, BigEndianU64, ParseError, Parser, Stream, StringsBlock,
};

pub struct AlignedParser<'a> {
    stream: Stream<'a, u32>,
    strings: StringsBlock<'a>,
}

impl Clone for AlignedParser<'_> {
    fn clone(&self) -> Self {
        Self { stream: self.stream.clone(), strings: self.strings.clone() }
    }
}

impl sealed::Sealed for AlignedParser<'_> {}
impl<'a> Parser<'a> for AlignedParser<'a> {
    type Granularity = u32;

    fn new(data: &'a [Self::Granularity], strings: &'a [u8]) -> Self {
        Self { stream: Stream::new(data), strings: StringsBlock(strings) }
    }

    fn data(&self) -> &'a [Self::Granularity] {
        self.stream.0
    }

    fn byte_data(&self) -> &'a [u8] {
        // SAFETY: it is always valid to cast a `u32` to 4 `u8`s
        unsafe {
            core::slice::from_raw_parts(
                self.stream.0.as_ptr().cast::<u8>(),
                core::mem::size_of_val(self.stream.0),
            )
        }
    }

    fn strings(&self) -> super::StringsBlock<'a> {
        self.strings
    }

    fn advance_token(&mut self) -> Result<BigEndianToken, ParseError> {
        loop {
            match BigEndianToken(
                self.stream.advance().map(BigEndianU32).ok_or(ParseError::UnexpectedEndOfData)?,
            ) {
                BigEndianToken::NOP => continue,
                token @ BigEndianToken::BEGIN_NODE
                | token @ BigEndianToken::END_NODE
                | token @ BigEndianToken::PROP
                | token @ BigEndianToken::END => break Ok(token),
                _ => break Err(ParseError::InvalidTokenValue),
            }
        }
    }

    fn advance_u32(&mut self) -> Result<BigEndianU32, ParseError> {
        self.stream.advance().map(BigEndianU32).ok_or(ParseError::UnexpectedEndOfData)
    }

    fn advance_u64(&mut self) -> Result<BigEndianU64, ParseError> {
        let (a, b) = self
            .stream
            .advance()
            .map(BigEndianU32)
            .zip(self.stream.advance().map(BigEndianU32))
            .ok_or(ParseError::UnexpectedEndOfData)?;

        #[cfg(target_endian = "little")]
        return Ok(BigEndianU64::from_be((u64::from(b.to_be()) << 32) | u64::from(a.to_be())));

        #[cfg(target_endian = "big")]
        return Ok(BigEndianU64::from_be((u64::from(a.to_be()) << 32) | u64::from(b.to_be())));
    }

    fn advance_cstr(&mut self) -> Result<&'a core::ffi::CStr, ParseError> {
        // SAFETY: It is safe to reinterpret the stream data to a smaller integer size
        let bytes = unsafe {
            core::slice::from_raw_parts(
                self.stream.0.as_ptr().cast::<u8>(),
                core::mem::size_of_val(self.stream.0),
            )
        };
        let cstr = core::ffi::CStr::from_bytes_until_nul(bytes)
            .map_err(|_| ParseError::InvalidCStrValue)?;

        // Round up to the next multiple of 4, if necessary
        let skip = ((cstr.to_bytes().len() + 3) & !3) / 4;
        self.stream.skip_many(skip);

        Ok(cstr)
    }

    fn advance_aligned(&mut self, n: usize) {
        // Round up to the next multiple of 4, if necessary
        let skip = ((n + 3) & !3) / 4;
        self.stream.skip_many(skip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advance_u64() {
        let n = BigEndianU64::from_ne(0xF00DCAFEDEADFEED);
        let mut parser = AlignedParser::new(
            unsafe { core::slice::from_raw_parts(&n as *const BigEndianU64 as *const u32, 2) },
            &[],
        );
        let m = parser.advance_u64().unwrap();

        assert_eq!(n, m);
    }
}
