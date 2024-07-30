use crate::{parsing::BigEndianU32, FdtError};
use core::ffi::CStr;

#[derive(Debug, Clone, Copy)]
pub struct InvalidPropertyValue;

impl From<InvalidPropertyValue> for FdtError {
    fn from(_: InvalidPropertyValue) -> Self {
        FdtError::InvalidPropertyValue
    }
}

pub trait PropertyValue<'a>: Sized {
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue>;
}

impl<'a> PropertyValue<'a> for u32 {
    #[inline]
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue> {
        match value {
            [a, b, c, d] => Ok(u32::from_be_bytes([*a, *b, *c, *d])),
            _ => Err(InvalidPropertyValue),
        }
    }
}

impl<'a> PropertyValue<'a> for u64 {
    #[inline]
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue> {
        match value {
            [a, b, c, d] => Ok(u64::from_be_bytes([0, 0, 0, 0, *a, *b, *c, *d])),
            [a, b, c, d, e, f, g, h] => Ok(u64::from_be_bytes([*a, *b, *c, *d, *e, *f, *g, *h])),
            _ => Err(InvalidPropertyValue),
        }
    }
}

impl<'a> PropertyValue<'a> for usize {
    #[inline]
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue> {
        #[cfg(target_pointer_width = "32")]
        let ret = match value {
            [a, b, c, d] => Ok(usize::from_be_bytes([*a, *b, *c, *d])),
            _ => Err(InvalidPropertyValue),
        };

        #[cfg(target_pointer_width = "64")]
        let ret = match value {
            [a, b, c, d] => Ok(usize::from_be_bytes([0, 0, 0, 0, *a, *b, *c, *d])),
            [a, b, c, d, e, f, g, h] => Ok(usize::from_be_bytes([*a, *b, *c, *d, *e, *f, *g, *h])),
            _ => Err(InvalidPropertyValue),
        };

        ret
    }
}

impl<'a> PropertyValue<'a> for BigEndianU32 {
    #[inline]
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue> {
        match value {
            [a, b, c, d] => Ok(BigEndianU32::from_be(u32::from_ne_bytes([*a, *b, *c, *d]))),
            _ => Err(InvalidPropertyValue),
        }
    }
}

impl<'a> PropertyValue<'a> for &'a CStr {
    #[inline]
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue> {
        CStr::from_bytes_until_nul(value).map_err(|_| InvalidPropertyValue)
    }
}

impl<'a> PropertyValue<'a> for &'a str {
    #[inline]
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue> {
        core::str::from_utf8(value).map(|s| s.trim_end_matches('\0')).map_err(|_| InvalidPropertyValue)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct U32List<'a>(&'a [u8]);

impl<'a> U32List<'a> {
    pub fn iter(self) -> U32ListIter<'a> {
        U32ListIter(self.0)
    }
}

impl<'a> PropertyValue<'a> for U32List<'a> {
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue> {
        if value.len() % 4 != 0 {
            return Err(InvalidPropertyValue);
        }

        Ok(Self(value))
    }
}

pub struct U32ListIter<'a>(&'a [u8]);

impl<'a> Iterator for U32ListIter<'a> {
    type Item = u32;
    fn next(&mut self) -> Option<Self::Item> {
        let val = u32::from_be_bytes(self.0.get(..4)?.try_into().unwrap());
        self.0 = self.0.get(4..)?;
        Some(val)
    }
}

#[derive(Debug, Clone)]
pub struct StringList<'a> {
    strs: core::str::Split<'a, char>,
}

impl<'a> PropertyValue<'a> for StringList<'a> {
    #[inline]
    fn parse(value: &'a [u8]) -> Result<Self, InvalidPropertyValue> {
        Ok(Self { strs: <&'a str as PropertyValue<'a>>::parse(value)?.split('\0') })
    }
}

impl<'a> Iterator for StringList<'a> {
    type Item = &'a str;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        self.strs.next()
    }
}
