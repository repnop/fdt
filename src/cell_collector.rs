use crate::FdtError;

#[derive(Debug, Clone, Copy)]
pub struct CollectCellsError;

impl From<CollectCellsError> for FdtError {
    fn from(_: CollectCellsError) -> Self {
        FdtError::CollectCellsError
    }
}

pub trait BuildCellCollector: Default {
    type Output;

    fn push(&mut self, component: u32) -> Result<(), CollectCellsError>;
    fn finish(self) -> Self::Output;
}

pub trait CellCollector: Default + Sized {
    type Output;
    type Builder: BuildCellCollector;

    fn map(builder_out: <Self::Builder as BuildCellCollector>::Output) -> Self::Output;
}

pub struct BuildIntCollector<Int> {
    value: Int,
}

impl<Int: Default> Default for BuildIntCollector<Int> {
    fn default() -> Self {
        Self { value: Default::default() }
    }
}

impl<
        Int: Copy
            + Default
            + core::cmp::PartialEq
            + core::ops::Shl<u32, Output = Int>
            + core::ops::Shr<u32, Output = Int>
            + core::ops::BitOr<Int, Output = Int>
            + From<u32>,
    > BuildCellCollector for BuildIntCollector<Int>
{
    type Output = Int;

    #[inline(always)]
    fn push(&mut self, component: u32) -> Result<(), CollectCellsError> {
        let shr = const {
            match core::mem::size_of::<Int>().checked_sub(4) {
                Some(value) => value as u32 * 8,
                None => panic!("integer type too small"),
            }
        };

        if self.value >> shr != Int::from(0u32) {
            return Err(CollectCellsError);
        }

        self.value = self.value.shl(32).bitor(Int::from(component));

        Ok(())
    }

    #[inline(always)]
    fn finish(self) -> Self::Output {
        self.value
    }
}

pub struct BuildWrappingIntCollector<Int> {
    value: Int,
}

impl<Int: Default> Default for BuildWrappingIntCollector<Int> {
    fn default() -> Self {
        Self { value: Default::default() }
    }
}

impl<Int: Copy + Default + core::ops::Shl<u32, Output = Int> + core::ops::BitOr<Int, Output = Int> + From<u32>>
    BuildCellCollector for BuildWrappingIntCollector<Int>
{
    type Output = Int;

    #[inline(always)]
    fn push(&mut self, component: u32) -> Result<(), CollectCellsError> {
        self.value = self.value.shl(32).bitor(Int::from(component));

        Ok(())
    }

    #[inline(always)]
    fn finish(self) -> Self::Output {
        self.value
    }
}

impl CellCollector for u32 {
    type Output = Self;
    type Builder = BuildIntCollector<Self>;

    #[inline(always)]
    fn map(builder_out: <BuildIntCollector<Self> as BuildCellCollector>::Output) -> Self::Output {
        builder_out
    }
}

impl CellCollector for u64 {
    type Output = Self;
    type Builder = BuildIntCollector<Self>;

    #[inline(always)]
    fn map(builder_out: <BuildIntCollector<Self> as BuildCellCollector>::Output) -> Self::Output {
        builder_out
    }
}

impl CellCollector for u128 {
    type Output = Self;
    type Builder = BuildIntCollector<Self>;

    #[inline(always)]
    fn map(builder_out: <BuildIntCollector<Self> as BuildCellCollector>::Output) -> Self::Output {
        builder_out
    }
}

impl CellCollector for usize {
    type Output = Self;
    type Builder = UsizeCollector;

    #[inline(always)]
    fn map(builder_out: <UsizeCollector as BuildCellCollector>::Output) -> Self::Output {
        builder_out
    }
}

impl<T: CellCollector> CellCollector for Option<T> {
    type Builder = BuildOptionalCellCollector<T>;
    type Output = Option<T::Output>;

    fn map(builder_out: <Self::Builder as BuildCellCollector>::Output) -> Self::Output {
        builder_out.map(T::map)
    }
}

#[derive(Default)]
pub struct UsizeCollector {
    value: usize,
}

impl BuildCellCollector for UsizeCollector {
    type Output = usize;

    #[inline(always)]
    fn push(&mut self, component: u32) -> Result<(), CollectCellsError> {
        use core::ops::{BitOr, Shl};

        let shr = const {
            match core::mem::size_of::<usize>().checked_sub(4) {
                Some(value) => value as u32 * 8,
                None => panic!("integer type too small"),
            }
        };

        if self.value >> shr != 0 {
            return Err(CollectCellsError);
        }

        self.value = self.value.shl(32i32).bitor(component as usize);

        Ok(())
    }

    #[inline(always)]
    fn finish(self) -> Self::Output {
        self.value
    }
}

pub struct BuildOptionalCellCollector<T: CellCollector> {
    builder: T::Builder,
    used: bool,
}

impl<T: CellCollector> Default for BuildOptionalCellCollector<T> {
    fn default() -> Self {
        Self { builder: Default::default(), used: false }
    }
}

impl<T: CellCollector> BuildCellCollector for BuildOptionalCellCollector<T> {
    type Output = Option<<T::Builder as BuildCellCollector>::Output>;

    #[inline(always)]
    fn push(&mut self, component: u32) -> Result<(), CollectCellsError> {
        self.used = true;
        self.builder.push(component)?;

        Ok(())
    }

    #[inline(always)]
    fn finish(self) -> Self::Output {
        match self.used {
            true => Some(self.builder.finish()),
            false => None,
        }
    }
}

impl<Int: Copy + Default + core::ops::Shl<u32, Output = Int> + core::ops::BitOr<Int, Output = Int> + From<u32>>
    CellCollector for core::num::Wrapping<Int>
{
    type Output = Int;
    type Builder = BuildWrappingIntCollector<Int>;

    #[inline(always)]
    fn map(builder_out: <Self::Builder as BuildCellCollector>::Output) -> Self::Output {
        builder_out
    }
}
