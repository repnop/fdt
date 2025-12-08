use crate::FdtError;

/// Error type indicating that the cell value that was attempted to be collected
/// was too large for the desired type.
#[derive(Debug, Clone, Copy)]
pub struct CollectCellsError;

impl From<CollectCellsError> for FdtError {
    fn from(_: CollectCellsError) -> Self {
        FdtError::CollectCellsError
    }
}

/// A type which performs the underlying collection of cell-sized values into
/// the desired underlying type.
pub trait BuildCellCollector: Default {
    /// Output of the builder. Usually the same as the type implementing
    /// [`CellCollector`].
    type Output;

    /// Push a new [`u32`] component of a cell-sized value. This can error
    /// whenever the value would overflow or otherwise be undesirable.
    fn push(&mut self, component: u32) -> Result<(), CollectCellsError>;
    /// Finish collecting components and return the collected value.
    fn finish(self) -> Self::Output;
}

/// A type which can be "collected" into from devicetree cell-sized values. The
/// most common types to use for this purpose are [`u32`] and [`u64`] but other
/// types may implement this trait when it's useful, such as [PciAddress].
///
/// In the case that a cell value may not exist (such as the parent unit address
/// in a PCI `interrupt-map`), [`Option<T>`] implements [`CellCollector`] for
/// any type `C: CellCollector`.
///
/// For those who want the collection of these values to always succeed,
/// [`core::num::Wrapping<T>`] implements [`CellCollector`] for numeric types
/// which fit the bounds. ([`u32`] and above, as it requires `From<u32>`)
///
/// [PciAddress]: crate::properties::interrupts::pci::PciAddress
pub trait CellCollector: Default + Sized {
    /// Underlying output type, this is usually the same as `Self`.
    type Output;
    /// Builder type used to collect the individual cell values into the desired type.
    type Builder: BuildCellCollector;

    /// Maps the builder output to the desired underlying type. This is usually
    /// a no-op, but may not always be, see the [`core::num::Wrapping<T>`] impl.
    fn map(builder_out: <Self::Builder as BuildCellCollector>::Output) -> Self::Output;
}

/// Generic integer type collector.
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

        // HACK: shifting a `u32` by `32` bits at all, regardless of the value,
        // panics, so for `u32`s, don't shift at all since the next call will
        // fail above.
        let shl = const {
            match core::mem::size_of::<Int>() {
                0..=4 => 0,
                _ => 32,
            }
        };

        self.value = self.value.shl(shl).bitor(Int::from(component));

        Ok(())
    }

    #[inline(always)]
    fn finish(self) -> Self::Output {
        self.value
    }
}

/// Wrapping collector, used for [`core::num::Wrapping<T>`].
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

#[allow(missing_docs)]
#[derive(Default)]
pub struct UsizeCollector {
    value: usize,
}

impl BuildCellCollector for UsizeCollector {
    type Output = usize;

    #[inline(always)]
    fn push(&mut self, component: u32) -> Result<(), CollectCellsError> {
        use core::ops::{BitOr, Shl};

        let shr = const { (core::mem::size_of::<usize>() - 4) * 8 };

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

/// [`BuildCellCollector`] for [`Option<T>`].
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
