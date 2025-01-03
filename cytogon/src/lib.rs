use std::ops::{Range, RangeInclusive};

pub use glam::{IVec2, IVec3, UVec2, UVec3};
use rand::{Rng, RngCore};
#[cfg(feature = "trace")]
use tracing::info_span;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
mod avx2;

/// Bitset encoding a rule for a 2D cellular automaton.
///
/// Each bit represents whether the associated rule applies to a cell with the
/// corresponding number of neighbors. The first / lowest bit corresponds to 0
/// neighbor, the next bit to 1 neighbor, etc. until the maximum number of Moore
/// neighbors (8).
///
/// # Construction
///
/// A new [`RuleBiset2`] can be constructed from:
/// - Another instance (`Copy`).
/// - A `u16` bit representation.
///
///   ```
///   let r = RuleBitset2::from(0xF07u32);
///   ```
/// - An array of exactly 9 `bool`.
///
///   ```
///   let r = RuleBitset2::from([true; 9]);
///   ```
/// - A slice of at most 9 `bool` (all missing elements are assumed `false`).
///
///   ```
///   let r = RuleBitset2::from(&[true; 5]);
///   ```
/// - A `Range<u8>` or `RangeInclusive<u8>` of length up to 9, describing the
///   `true` values.
///
///   ```
///   let r = RuleBitset2::from(3u8..8u8);
///   let r = RuleBitset2::from(3u8..=7u8);
///   ```
///
/// - By combining 2 existing rules via the bitwise OR `|` operator.
///
///   ```
///   let r1 = RuleBitset2::from(1u8..4u8);
///   let r2 = RuleBitset2::from(7u8..=8u8);
///   let r = r1 | r2;
///   ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct RuleBitset2(u16);

impl RuleBitset2 {
    /// Create a rule bitset from a bit representation.
    ///
    /// In general, prefer using `From<u16>` for clarity and conciseness. This
    /// is mainly provided as fallback for `const` context.
    pub const fn from_bits(bits: u16) -> Self {
        assert!(
            bits & 0xFE00u16 == 0,
            "Invalid bit pattern: the top 7 bits must be zero."
        );
        Self(bits)
    }

    /// Create a rule bitset by OR'ing two existing bit representations.
    ///
    /// In general, prefer using the bitwise OR `|` operator for clarity and
    /// conciseness. This is mainly provided as fallback for `const`
    /// context.
    pub const fn or_with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Convert the rule to its bit representation.
    ///
    /// The returned `u16` always has its upper 7 bits set to zero. The lower 9
    /// bits represent whether the associated rule applies to a cell with 0 to
    /// 8 neighbors.
    #[inline]
    pub fn to_bits(&self) -> u16 {
        self.0
    }

    /// Convert the rule to an array of `bool`s.
    ///
    /// The `bool` at index N corresponds to the N-th bit (from lowest to
    /// highest) in the bit representation. The bits represent whether the
    /// associated rule applies to a cell with 0 to 8 neighbors.
    pub fn to_array(&self) -> [bool; 9] {
        [
            self.0 & 0x1 != 0,
            self.0 & 0x2 != 0,
            self.0 & 0x4 != 0,
            self.0 & 0x8 != 0,
            self.0 & 0x10 != 0,
            self.0 & 0x20 != 0,
            self.0 & 0x40 != 0,
            self.0 & 0x80 != 0,
            self.0 & 0x100 != 0,
        ]
    }
}

impl From<u16> for RuleBitset2 {
    #[inline]
    fn from(value: u16) -> Self {
        Self(value)
    }
}

impl From<Range<u8>> for RuleBitset2 {
    fn from(value: Range<u8>) -> Self {
        assert!(value.end <= 8);
        let mut bits = 0;
        for b in value {
            bits |= 1u16 << b;
        }
        Self(bits)
    }
}

impl From<RangeInclusive<u8>> for RuleBitset2 {
    fn from(value: RangeInclusive<u8>) -> Self {
        assert!(*value.end() <= 8);
        let mut bits = 0;
        for b in value {
            bits |= 1u16 << b;
        }
        Self(bits)
    }
}

impl From<[bool; 9]> for RuleBitset2 {
    fn from(value: [bool; 9]) -> Self {
        let mut bit = 0;
        let bits = value.iter().fold(0u16, move |acc, &b| {
            let b = (b as u16) << bit;
            bit += 1;
            acc | b
        });
        Self(bits)
    }
}

impl From<&[bool]> for RuleBitset2 {
    fn from(value: &[bool]) -> Self {
        assert!(value.len() <= 9);
        let mut bit = 0;
        let bits = value.iter().fold(0u16, move |acc, &b| {
            let b = (b as u16) << bit;
            bit += 1;
            acc | b
        });
        Self(bits)
    }
}

impl std::ops::BitOr for RuleBitset2 {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.to_bits() | rhs.to_bits())
    }
}

/// 2D cellular automaton rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule2 {
    /// Birth rule, applied to dead cells to determine if they become alive.
    pub birth: RuleBitset2,
    /// Survive rule, applied to alive cells to determine if they remain alive.
    pub survive: RuleBitset2,
}

impl Rule2 {
    /// Smoothing rule S4-8/B5-8/2/M.
    pub const SMOOTH: Rule2 = Rule2 {
        birth: RuleBitset2::from_bits(0x1E0u16),   // 5..=8
        survive: RuleBitset2::from_bits(0x1F0u16), // 4..=8
    };

    /// Create a CA rule from a pair of birth and survive rules.
    pub fn new(birth: impl Into<RuleBitset2>, survive: impl Into<RuleBitset2>) -> Self {
        Self {
            birth: birth.into(),
            survive: survive.into(),
        }
    }
}

/// 2D cellular automaton grid.
///
/// Each cell in the grid is encoded as a boolean or bit, and can be alive
/// (`true` / `1`) or dead (`false` / `0`).
#[derive(Clone)]
pub struct Grid2 {
    /// Grid size, in number of cells.
    pub size: UVec2,
    /// Bitblocks encoding the state of all cells in the grid.
    ///
    /// The bitblocks are laid out in X-major order, that is all X blocks for
    /// Y=0, then all X blocks for Y=1, etc. There are
    /// [`Self::get_bitblock_count`]`(`[`Self::size`]`)` blocks.
    pub data: Vec<u64>,
}

impl Grid2 {
    pub fn new(size: UVec2) -> Self {
        Self { size, data: vec![] }
    }

    /// Get the number of bit blocks to allocate for a given grid size.
    ///
    /// A bit block is a single `u64` value encoding the alive/dead state of a
    /// 8x8 block of 64 cells.
    #[inline]
    fn get_bitblock_count(size: UVec2) -> usize {
        size.x.div_ceil(8) as usize * size.y.div_ceil(8) as usize
    }

    /// Fill the grid with the given `value`.
    pub fn fill(&mut self, value: bool) {
        #[cfg(feature = "trace")]
        let _span = info_span!("fill2").entered();

        let size = Self::get_bitblock_count(self.size);
        let value = if value { !0u64 } else { 0 };
        self.data.resize(size, value);
    }

    /// Fill the grid with random values.
    ///
    /// The fill ratio determines how "full" the grid is, that is the proportion
    /// of alive cells.
    pub fn fill_rand(&mut self, fill_ratio: f32, mut prng: impl RngCore) {
        #[cfg(feature = "trace")]
        let _span = info_span!("fill_rand2").entered();

        let capacity = Self::get_bitblock_count(self.size);
        self.data = fill_rand(capacity, fill_ratio, &mut prng);
    }

    #[inline]
    pub fn cell(&self, pos: IVec2) -> Option<bool> {
        if pos.x < 0 || pos.y < 0 || pos.x as u32 >= self.size.x || pos.y as u32 >= self.size.y {
            None
        } else {
            let index = pos.y as u32 * self.size.x + pos.x as u32;
            let i0 = index >> 6;
            let i1 = 1u64 << (index & 0x3F);
            Some(self.data[i0 as usize] & i1 != 0)
        }
    }

    #[inline]
    pub fn set_cell(&mut self, pos: IVec2, value: bool) {
        if pos.x < 0 || pos.y < 0 || pos.x as u32 >= self.size.x || pos.y as u32 >= self.size.y {
        } else {
            let index = pos.y as u32 * self.size.x + pos.x as u32;
            let i0 = index >> 6;
            let i1 = (1u64 as u64) << (index & 0x3F);
            if value {
                self.data[i0 as usize] |= i1;
            } else {
                self.data[i0 as usize] &= !i1;
            }
        }
    }

    pub fn apply_rule(&mut self, rule: &Rule2) {
        #[cfg(feature = "trace")]
        let _span = info_span!("apply_rule2").entered();

        let imax = self.size.x - 1;
        let jmax = self.size.y - 1;
        let default = false;
        let old_grid = self.clone();
        let survive = rule.survive.to_array();
        let birth = rule.birth.to_array();
        for j in 0..=jmax {
            for i in 0..=imax {
                let pos = IVec2::new(i as i32, j as i32);
                if default && (i == 0 || j == 0 || i == imax || j == jmax) {
                    self.set_cell(pos, true);
                } else {
                    let c = old_grid.count_neighbors(pos, default);
                    if self.cell(pos).unwrap_or(false) {
                        if !survive[c as usize] {
                            self.set_cell(pos, false);
                        }
                    } else {
                        if birth[c as usize] {
                            self.set_cell(pos, true);
                        }
                    };
                }
            }
        }
    }

    /// Count the number of alive neighbor cells at the given position.
    ///
    /// If the position is on the edges of the grid, assume some neighbor exists
    /// outside the grid with a value of `default`. For `default = false`, this
    /// counts the actual neighbors without any change. For `default = true`,
    /// this adds a virtual boundary condition around the grid. This is useful
    /// to ensure the resulting geometry is closed.
    fn count_neighbors(&self, pos: IVec2, default: bool) -> u8 {
        let mut count = 0;
        let mut xy = pos;
        for j in (pos.y - 1)..=(pos.y + 1) {
            xy.y = j;
            for i in (pos.x - 1)..=(pos.x + 1) {
                xy.x = i;
                if xy != pos && self.cell(xy).unwrap_or(default) {
                    count += 1;
                }
            }
        }
        count
    }
}

/// Bitset encoding a rule for a 3D cellular automaton.
///
/// Each bit represents whether the associated rule applies to a cell with the
/// corresponding number of neighbors. The first / lowest bit corresponds to 0
/// neighbor, the next bit to 1 neighbor, etc. until the maximum number of Moore
/// neighbors (26).
///
/// # Construction
///
/// A new [`RuleBiset3`] can be constructed from:
/// - Another instance (`Copy`).
/// - A `u32` bit representation.
///
///   ```
///   let r = RuleBitset3::from(0xF07u32);
///   ```
/// - An array of exactly 27 `bool`.
///
///   ```
///   let r = RuleBitset3::from([true; 27]);
///   ```
/// - A slice of at most 27 `bool` (all missing elements are assumed `false`).
///
///   ```
///   let r = RuleBitset3::from(&[true; 14]);
///   ```
/// - A `Range<u8>` or `RangeInclusive<u8>` of length up to 27, describing the
///   `true` values.
///
///   ```
///   let r = RuleBitset3::from(3u8..17u8);
///   let r = RuleBitset3::from(13u8..=16u8);
///   ```
///
/// - By combining 2 existing rules via the bitwise OR `|` operator.
///
///   ```
///   let r1 = RuleBitset3::from(3u8..8u8);
///   let r2 = RuleBitset3::from(13u8..=16u8);
///   let r = r1 | r2;
///   ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct RuleBitset3(u32);

impl RuleBitset3 {
    /// Create a rule bitset from a bit representation.
    ///
    /// In general, prefer using `From<u32>` for clarity and conciseness. This
    /// is mainly provided as fallback for `const` context.
    pub const fn from_bits(bits: u32) -> Self {
        assert!(
            bits & 0xF800_0000u32 == 0,
            "Invalid bit pattern: the top 5 bits must be zero."
        );
        Self(bits)
    }

    /// Create a rule bitset by OR'ing two existing bit representations.
    ///
    /// In general, prefer using the bitwise OR `|` operator for clarity and
    /// conciseness. This is mainly provided as fallback for `const`
    /// context.
    pub const fn or_with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Convert the rule to its bit representation.
    ///
    /// The returned `u32` always has its upper 5 bits set to zero. The lower 27
    /// bits represent whether the associated rule applies to a cell with 0 to
    /// 26 neighbors.
    #[inline]
    pub fn to_bits(&self) -> u32 {
        self.0
    }

    /// Convert the rule to an array of `bool`s.
    ///
    /// The `bool` at index N corresponds to the N-th bit (from lowest to
    /// highest) in the bit representation. The bits represent whether the
    /// associated rule applies to a cell with 0 to 26 neighbors.
    pub fn to_array(&self) -> [bool; 27] {
        [
            self.0 & 0x1 != 0,
            self.0 & 0x2 != 0,
            self.0 & 0x4 != 0,
            self.0 & 0x8 != 0,
            self.0 & 0x10 != 0,
            self.0 & 0x20 != 0,
            self.0 & 0x40 != 0,
            self.0 & 0x80 != 0,
            self.0 & 0x100 != 0,
            self.0 & 0x200 != 0,
            self.0 & 0x400 != 0,
            self.0 & 0x800 != 0,
            self.0 & 0x1000 != 0,
            self.0 & 0x2000 != 0,
            self.0 & 0x4000 != 0,
            self.0 & 0x8000 != 0,
            self.0 & 0x1_0000 != 0,
            self.0 & 0x2_0000 != 0,
            self.0 & 0x4_0000 != 0,
            self.0 & 0x8_0000 != 0,
            self.0 & 0x10_0000 != 0,
            self.0 & 0x20_0000 != 0,
            self.0 & 0x40_0000 != 0,
            self.0 & 0x80_0000 != 0,
            self.0 & 0x100_0000 != 0,
            self.0 & 0x200_0000 != 0,
            self.0 & 0x400_0000 != 0,
        ]
    }
}

impl From<u32> for RuleBitset3 {
    #[inline]
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl From<Range<u8>> for RuleBitset3 {
    fn from(value: Range<u8>) -> Self {
        assert!(value.end <= 27);
        let mut bits = 0;
        for b in value {
            bits |= 1u32 << b;
        }
        Self(bits)
    }
}

impl From<RangeInclusive<u8>> for RuleBitset3 {
    fn from(value: RangeInclusive<u8>) -> Self {
        assert!(*value.end() <= 27);
        let mut bits = 0;
        for b in value {
            bits |= 1u32 << b;
        }
        Self(bits)
    }
}

impl From<[bool; 27]> for RuleBitset3 {
    fn from(value: [bool; 27]) -> Self {
        let mut bit = 0;
        let bits = value.iter().fold(0u32, move |acc, &b| {
            let b = (b as u32) << bit;
            bit += 1;
            acc | b
        });
        Self(bits)
    }
}

impl From<&[bool]> for RuleBitset3 {
    fn from(value: &[bool]) -> Self {
        assert!(value.len() <= 27);
        let mut bit = 0;
        let bits = value.iter().fold(0u32, move |acc, &b| {
            let b = (b as u32) << bit;
            bit += 1;
            acc | b
        });
        Self(bits)
    }
}

impl std::ops::BitOr for RuleBitset3 {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.to_bits() | rhs.to_bits())
    }
}

/// 3D cellular automaton rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rule3 {
    /// Birth rule, applied to dead cells to determine if they become alive.
    pub birth: RuleBitset3,
    /// Survive rule, applied to alive cells to determine if they remain alive.
    pub survive: RuleBitset3,
}

impl Rule3 {
    /// Smoothing rule S13-26/B13-14,17-19/2/M.
    pub const SMOOTH: Rule3 = Rule3 {
        birth: RuleBitset3::from_bits(0xE_6000u32), // 13..=14, 17..=19
        survive: RuleBitset3::from_bits(0x7FF_E000u32), // 13..=26
    };

    /// Create a CA rule from a pair of birth and survive rules.
    pub fn new(birth: impl Into<RuleBitset3>, survive: impl Into<RuleBitset3>) -> Self {
        Self {
            birth: birth.into(),
            survive: survive.into(),
        }
    }
}

/// 3D cellular automaton grid.
///
/// Each cell in the grid is encoded as a boolean or bit, and can be alive
/// (`true` / `1`) or dead (`false` / `0`).
#[derive(Clone)]
pub struct Grid3 {
    /// Grid size, in number of cells.
    pub size: UVec3,
    /// Bitblocks encoding the state of all cells in the grid.
    ///
    /// The bitblocks are laid out in X-major and Z-minor order, that is all X
    /// blocks for Y=Z=0, then all X blocks for Z=0 and Y=1, etc. There are
    /// [`Self::get_bitblock_count`]`(`[`Self::size`]`)` blocks.
    pub data: Vec<u64>,
}

impl Grid3 {
    /// Create a new empty grid of the given size.
    ///
    /// To save on allocations, the grid is not allocated until one of the
    /// [`fill()`] or [`fill_rand()`] functions are called.
    pub fn new(size: UVec3) -> Self {
        Self { size, data: vec![] }
    }

    /// Get the number of bit blocks to allocate for a given grid size.
    ///
    /// A bit block is a single `u64` value encoding the alive/dead state of a
    /// 4x4x4 block of 64 cells.
    #[inline]
    fn get_bitblock_count(size: UVec3) -> usize {
        size.x.div_ceil(4) as usize * size.y.div_ceil(4) as usize * size.z.div_ceil(4) as usize
    }

    /// Fill the grid with the given `value`.
    pub fn fill(&mut self, value: bool) {
        #[cfg(feature = "trace")]
        let _span = info_span!("fill3").entered();

        let size = Self::get_bitblock_count(self.size);
        let value = if value { !0u64 } else { 0 };
        self.data.resize(size, value);
    }

    /// Fill the grid with random values.
    ///
    /// The fill ratio determines how "full" the grid is, that is the proportion
    /// of alive cells.
    pub fn fill_rand(&mut self, fill_ratio: f32, mut prng: impl RngCore) {
        #[cfg(feature = "trace")]
        let _span = info_span!("fill_rand3").entered();

        let capacity = Self::get_bitblock_count(self.size);
        self.data = fill_rand(capacity, fill_ratio, &mut prng);
    }

    /// Resolve the position of a cell in the grid to its array index and bit.
    fn resolve(&self, pos: IVec3) -> Option<(usize, u8)> {
        if pos.x < 0
            || pos.y < 0
            || pos.z < 0
            || pos.x as u32 >= self.size.x
            || pos.y as u32 >= self.size.y
            || pos.z as u32 >= self.size.z
        {
            None
        } else {
            let index = (pos.z / 4) as u32 * (self.size.y / 4) * (self.size.x / 4)
                + (pos.y / 4) as u32 * (self.size.x / 4)
                + (pos.x / 4) as u32;
            let bit = (pos.x as u8 & 0x3) | ((pos.y as u8 & 0x3) << 2) | ((pos.z as u8 & 0x3) << 4);
            Some((index as usize, bit))
        }
    }

    /// Resolve the position of a cell in the grid to its array index and bit.
    #[inline]
    fn resolve_bit(&self, pos: IVec3) -> Option<(usize, u64)> {
        self.resolve(pos).map(|(index, bit)| (index, 1u64 << bit))
    }

    #[inline]
    pub fn cell(&self, pos: IVec3) -> Option<bool> {
        if let Some((index, bit)) = self.resolve_bit(pos) {
            Some(self.data[index] & bit != 0)
        } else {
            None
        }
    }

    #[inline]
    pub fn set_cell(&mut self, pos: IVec3, value: bool) {
        if let Some((index, bit)) = self.resolve_bit(pos) {
            if value {
                self.data[index] |= bit;
            } else {
                self.data[index] &= !bit;
            }
        }
    }

    /// Apply the given cellular automaton rule once to the entire grid.
    pub fn apply_rule(&mut self, rule: &Rule3) {
        // #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        // {
        //     if is_x86_feature_detected!("avx2") {
        //         return unsafe { avx2::apply_rule(self.size, &self.data[..], default)
        // };     }
        // }

        self.apply_rule_ref(rule)
    }

    /// Reference single-threaded implementation of [`apply_rule()`]. Very slow.
    pub fn apply_rule_ref(&mut self, rule: &Rule3) {
        #[cfg(feature = "trace")]
        let _span = info_span!("apply_rule3").entered();

        let imax = self.size.x - 1;
        let jmax = self.size.y - 1;
        let kmax = self.size.z - 1;
        let default = false;
        let old_grid = self.clone();
        let counts = old_grid.count_neighbors(default);
        for k in 0..=kmax {
            for j in 0..=jmax {
                for i in 0..=imax {
                    let pos = IVec3::new(i as i32, j as i32, k as i32);
                    if default
                        && (i == 0 || j == 0 || k == 0 || i == imax || j == jmax || k == kmax)
                    {
                        self.set_cell(pos, true);
                    } else {
                        // 13-26/13-14,17-19/2/M
                        let cell = self.cell(pos).unwrap();
                        let (index, offset) = self.resolve(pos).unwrap();
                        let c = counts[index * 64 + offset as usize];
                        let b = 1u32 << c;
                        if cell {
                            self.set_cell(pos, rule.survive.to_bits() & b != 0);
                        } else {
                            self.set_cell(pos, rule.birth.to_bits() & b != 0);
                        }
                    }
                }
            }
        }
    }

    /// Count the number of alive neighbor cells at the given position.
    ///
    /// If the position is on the edges of the grid, assume some neighbor exists
    /// outside the grid with a value of `default`. For `default = false`, this
    /// counts the actual neighbors without any change. For `default = true`,
    /// this adds a virtual boundary condition around the grid. This is useful
    /// to ensure the resulting geometry is closed.
    fn count_neighbors(&self, default: bool) -> Vec<u8> {
        // #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        // {
        //     if is_x86_feature_detected!("avx2") {
        //         return unsafe { avx2::count_neighbors(self.size, &self.data[..],
        // default) };     }
        // }

        //self.count_neighbors_ref(default)

        self.count_neighbors_separable_m(default)
    }

    /// Convert the 8 lowest bits of the input to 8 bytes (lowest bit).
    ///
    /// This effectively "decompresses" the lowest 8 bits of a bitblock into 8
    /// byte values which can later be used for counting.
    #[inline]
    fn bit_to_byte(b: u64) -> u64 {
        // Copy the lowest byte into all 7 other ones
        let x = b | (b << 8);
        let x = x | (x << 16);
        let x = x | (x << 32);
        // Mask out all but 1 bit per byte, always a different one
        let x = x & 0x8040201008040201u64;
        // Move all bits back to the lowest bit.
        let y = (x & 0x8000_0000_0000_0000u64) >> 7
            | (x & 0x0040_0000_0000_0000u64) >> 6
            | (x & 0x0000_2000_0000_0000u64) >> 5
            | (x & 0x0000_0010_0000_0000u64) >> 4
            | (x & 0x0000_0000_0800_0000u64) >> 3
            | (x & 0x0000_0000_0004_0000u64) >> 2
            | (x & 0x0000_0000_0000_0200u64) >> 1
            | (x & 0x0000_0000_0000_0001u64);
        y
    }

    /// Decompress a 4x4x4 bitblock of 64 bits into an equivalent byte block.
    ///
    /// The layout of the input bitblock and the output byte block are the same.
    /// That is, the lowest bit decompresses to the lowest byte of the first
    /// `u64`, etc. On little endian platforms, the output array can be
    /// reinterpreted as `[u8; 64]` and will contain all bytes corresponding
    /// to the bits of the input, in LSB to MSB order.
    fn decompress_block(b: u64) -> [u64; 8] {
        [
            Self::bit_to_byte(b & 0xFFu64),
            Self::bit_to_byte((b & 0xFF00u64) >> 8),
            Self::bit_to_byte((b & 0xFF_0000u64) >> 16),
            Self::bit_to_byte((b & 0xFF00_0000u64) >> 24),
            Self::bit_to_byte((b & 0xFF_0000_0000u64) >> 32),
            Self::bit_to_byte((b & 0xFF00_0000_0000u64) >> 40),
            Self::bit_to_byte((b & 0xFF_0000_0000_0000u64) >> 48),
            Self::bit_to_byte((b & 0xFF00_0000_0000_0000u64) >> 56),
        ]
    }

    /// Count Moore 8-neighbors (or, 26 in 3D) with POPCNT.
    pub(crate) fn count_neighbors_popcnt_m(&self, default: bool) -> Vec<u8> {
        let block_count = self.size.as_ivec3() / 4;

        // Over-allocate entire blocks to avoid having to bound-check the writes
        let capacity =
            block_count.x as usize * block_count.y as usize * block_count.z as usize * 64;
        let mut counts = Vec::with_capacity(capacity);
        counts.resize(capacity, 0);

        // Loop over 2x2x2 chunks.
        let mut bpos = IVec3::ZERO;
        let dy = block_count.x as usize;
        let dz = (block_count.x * block_count.y) as usize;
        let mut ic = 0;
        for (ib, b) in self.data.iter().enumerate() {
            // If

            let xm = if bpos.x > 0 { self.data[ib - 1] } else { 0u64 };
            let xp = if bpos.x + 1 < block_count.x {
                self.data[ib + 1]
            } else {
                0u64
            };
            let ym = if bpos.y > 0 { self.data[ib - dy] } else { 0u64 };
            let yp = if bpos.y + 1 < block_count.y {
                self.data[ib + dy]
            } else {
                0u64
            };
            let zm = if bpos.z > 0 { self.data[ib - dz] } else { 0u64 };
            let zp = if bpos.z + 1 < block_count.z {
                self.data[ib + dz]
            } else {
                0u64
            };

            // Accumulate within the block
            let acc = [
                (*b & 0x0000_0000_0033_0033u64).count_ones() as u8,
                (*b & 0x0000_0000_0077_0077u64).count_ones() as u8,
                (*b & 0x0000_0000_00EE_00EEu64).count_ones() as u8,
                (*b & 0x0000_0000_00CC_00CCu64).count_ones() as u8,
                //
                (*b & 0x0000_0000_0333_0333u64).count_ones() as u8,
                (*b & 0x0000_0000_0777_0777u64).count_ones() as u8,
                (*b & 0x0000_0000_0EEE_0EEEu64).count_ones() as u8,
                (*b & 0x0000_0000_0CCC_0CCCu64).count_ones() as u8,
                //
                (*b & 0x0000_0000_3330_3330u64).count_ones() as u8,
                (*b & 0x0000_0000_7770_7770u64).count_ones() as u8,
                (*b & 0x0000_0000_EEE0_EEE0u64).count_ones() as u8,
                (*b & 0x0000_0000_CCC0_CCC0u64).count_ones() as u8,
                //
                (*b & 0x0000_0000_3300_3300u64).count_ones() as u8,
                (*b & 0x0000_0000_7700_7700u64).count_ones() as u8,
                (*b & 0x0000_0000_EE00_EE00u64).count_ones() as u8,
                (*b & 0x0000_0000_CC00_CC00u64).count_ones() as u8,
                // ----
                (*b & 0x0000_0033_0033_0033u64).count_ones() as u8,
                (*b & 0x0000_0077_0077_0077u64).count_ones() as u8,
                (*b & 0x0000_00EE_00EE_00EEu64).count_ones() as u8,
                (*b & 0x0000_00CC_00CC_00CCu64).count_ones() as u8,
                //
                (*b & 0x0000_0333_0333_0333u64).count_ones() as u8,
                (*b & 0x0000_0777_0777_0777u64).count_ones() as u8,
                (*b & 0x0000_0EEE_0EEE_0EEEu64).count_ones() as u8,
                (*b & 0x0000_0CCC_0CCC_0CCCu64).count_ones() as u8,
                //
                (*b & 0x0000_3330_3330_3330u64).count_ones() as u8,
                (*b & 0x0000_7770_7770_7770u64).count_ones() as u8,
                (*b & 0x0000_EEE0_EEE0_EEE0u64).count_ones() as u8,
                (*b & 0x0000_CCC0_CCC0_CCC0u64).count_ones() as u8,
                //
                (*b & 0x0000_3300_3300_3300u64).count_ones() as u8,
                (*b & 0x0000_7700_7700_7700u64).count_ones() as u8,
                (*b & 0x0000_EE00_EE00_EE00u64).count_ones() as u8,
                (*b & 0x0000_CC00_CC00_CC00u64).count_ones() as u8,
                // ----
                (*b & 0x0033_0033_0033_0000u64).count_ones() as u8,
                (*b & 0x0077_0077_0077_0000u64).count_ones() as u8,
                (*b & 0x00EE_00EE_00EE_0000u64).count_ones() as u8,
                (*b & 0x00CC_00CC_00CC_0000u64).count_ones() as u8,
                //
                (*b & 0x0333_0333_0333_0000u64).count_ones() as u8,
                (*b & 0x0777_0777_0777_0000u64).count_ones() as u8,
                (*b & 0x0EEE_0EEE_0EEE_0000u64).count_ones() as u8,
                (*b & 0x0CCC_0CCC_0CCC_0000u64).count_ones() as u8,
                //
                (*b & 0x3330_3330_3330_0000u64).count_ones() as u8,
                (*b & 0x7770_7770_7770_0000u64).count_ones() as u8,
                (*b & 0xEEE0_EEE0_EEE0_0000u64).count_ones() as u8,
                (*b & 0xCCC0_CCC0_CCC0_0000u64).count_ones() as u8,
                //
                (*b & 0x3300_3300_3300_0000u64).count_ones() as u8,
                (*b & 0x7700_7700_7700_0000u64).count_ones() as u8,
                (*b & 0xEE00_EE00_EE00_0000u64).count_ones() as u8,
                (*b & 0xCC00_CC00_CC00_0000u64).count_ones() as u8,
                // ----
                (*b & 0x0033_0033_0000_0000u64).count_ones() as u8,
                (*b & 0x0077_0077_0000_0000u64).count_ones() as u8,
                (*b & 0x00EE_00EE_0000_0000u64).count_ones() as u8,
                (*b & 0x00CC_00CC_0000_0000u64).count_ones() as u8,
                //
                (*b & 0x0333_0333_0000_0000u64).count_ones() as u8,
                (*b & 0x0777_0777_0000_0000u64).count_ones() as u8,
                (*b & 0x0EEE_0EEE_0000_0000u64).count_ones() as u8,
                (*b & 0x0CCC_0CCC_0000_0000u64).count_ones() as u8,
                //
                (*b & 0x3330_3330_0000_0000u64).count_ones() as u8,
                (*b & 0x7770_7770_0000_0000u64).count_ones() as u8,
                (*b & 0xEEE0_EEE0_0000_0000u64).count_ones() as u8,
                (*b & 0xCCC0_CCC0_0000_0000u64).count_ones() as u8,
                //
                (*b & 0x3300_3300_0000_0000u64).count_ones() as u8,
                (*b & 0x7700_7700_0000_0000u64).count_ones() as u8,
                (*b & 0xEE00_EE00_0000_0000u64).count_ones() as u8,
                (*b & 0xCC00_CC00_0000_0000u64).count_ones() as u8,
            ];

            // Copy counts into output array
            counts[ic..ic + 64].copy_from_slice(bytemuck::cast_slice(&acc[..]));
            ic += 64;

            // Update block position
            bpos.x += 1;
            if bpos.x >= block_count.x {
                bpos.x = 0;
                bpos.y += 1;
                if bpos.y >= block_count.y {
                    bpos.y = 0;
                    bpos.z += 1;
                }
            }
        }

        counts
    }

    fn acc_x(src: &[u64], counts: &mut [u8], block_count: IVec3) {
        debug_assert!(src.len() * 64 <= counts.len());
        let mut bpos = IVec3::ZERO;
        let mut ic = 0;
        for (ib, b) in src.iter().enumerate() {
            // Decompress the current block into 8 x 8 bytes. Each byte has its lowest bit
            // set or not.
            let b = Self::decompress_block(*b);

            // X
            let mut acc = b;
            for i in 0..8 {
                // x- : shift left, clear right face (x+)
                let xm = (b[i] >> 8) & 0x00FF_FFFF_00FF_FFFFu64;
                // x+ : shift right, clear left face (x-)
                let xp = (b[i] << 8) & 0xFFFF_FF00_FFFF_FF00u64;
                acc[i] += xm + xp;
            }

            // Fixup missing data from X- block, which is needed to fill left the current
            // block when it's shifted right.
            if bpos.x > 0 {
                let mut xm = Self::decompress_block(src[ib - 1]);
                for i in 0..8 {
                    // Move right face to left and clear the rest
                    xm[i] = (xm[i] >> 24) & 0x0000_00FF_0000_00FFu64;
                    // Accumulate
                    acc[i] += xm[i];
                }
            }

            // Fixup missing data from X+ block, which is needed to fill right the current
            // block when it's shifted left.
            if bpos.x + 1 < block_count.x {
                let mut xp = Self::decompress_block(src[ib + 1]);
                for i in 0..8 {
                    // Move left face to right and clear the rest
                    xp[i] = (xp[i] << 24) & 0xFF00_0000_FF00_0000u64;
                    // Accumulate
                    acc[i] += xp[i];
                }
            }

            // Copy counts into output array
            counts[ic..ic + 64].copy_from_slice(bytemuck::cast_slice(&acc[..]));
            ic += 64;

            // Update block position
            bpos.x += 1;
            if bpos.x >= block_count.x {
                bpos.x = 0;
                bpos.y += 1;
                if bpos.y >= block_count.y {
                    bpos.y = 0;
                    bpos.z += 1;
                }
            }
        }
    }

    fn acc_y(src: &[u8], dst: &mut [u8], block_count: IVec3) {
        let dy = block_count.x;
        let mut bpos = IVec3::ZERO;
        let mut ic = 0;
        for ib in 0..(src.len() / 64) {
            let prev: &[[u64; 8]] = bytemuck::cast_slice(&src[..]);
            let b = prev[ib];

            // Y
            let mut acc = b;
            for i in 0..8 {
                // y- : shift left (shifting pads with zero, auto-clears left face (y+))
                let mut ym = b[i] >> 32;
                if i & 0x1 == 0 {
                    // also move in the Y row from the adjacent accumulator
                    ym |= b[i + 1] << 32;
                }
                // y- : shift right (shifting pads with zero, auto-clears left face (y-))
                let mut yp = b[i] << 32;
                if i & 0x1 != 0 {
                    // also move in the Y row from the adjacent accumulator
                    yp |= b[i - 1] >> 32;
                }
                acc[i] += ym + yp;
            }

            // Fixup missing data from Y- block, which is needed to fill left the current
            // block when it's shifted right. Because a Y edge decompresses to 2
            // u64, one for the first 2 values and the next for the 2 others, we
            // actually read from the right one (n+1) and write into the left
            // one (n).
            if bpos.y > 0 {
                let ym = prev[ib - dy as usize];
                for i in 0..4 {
                    // Move right face to left and accumulate
                    acc[2 * i] += ym[2 * i + 1] >> 32;
                }
            }

            // Fixup missing data from Y+ block, which is needed to fill right the current
            // block when it's shifted left. Because a Y edge decompresses to 2
            // u64, one for the first 2 values and the next for the 2 others, we
            // actually read from the left one (n) and write into the right one
            // (n+1).
            if bpos.y + 1 < block_count.y {
                let yp = prev[ib + dy as usize];
                for i in 0..4 {
                    // Move left face to right and accumulate
                    acc[2 * i + 1] += yp[2 * i] << 32;
                }
            }

            // Copy counts into output array
            dst[ic..ic + 64].copy_from_slice(bytemuck::cast_slice(&acc[..]));
            ic += 64;

            // Update block position
            bpos.x += 1;
            if bpos.x >= block_count.x {
                bpos.x = 0;
                bpos.y += 1;
                if bpos.y >= block_count.y {
                    bpos.y = 0;
                    bpos.z += 1;
                }
            }
        }
    }

    fn acc_z(src: &[u8], dst: &mut [u8], block_count: IVec3) {
        let dz = block_count.x as usize * block_count.y as usize;
        let mut bpos = IVec3::ZERO;
        let mut ic = 0;
        for ib in 0..(src.len() / 64) {
            let prev: &[[u64; 8]] = bytemuck::cast_slice(&src[..]);
            let b = prev[ib];

            // Z
            let mut acc = b;
            for i in 2..8 {
                // z- : shift left
                acc[i] += b[i - 2];
            }
            for i in 0..6 {
                // z+ : shift right
                acc[i] += b[i + 2];
            }

            // Fixup missing data from Z- block, which is needed to fill left the current
            // block when it's shifted right. Because a Z face decompresses to 2
            // u64, we actually read from the entire face directly, and
            // accumulate.
            if bpos.z > 0 {
                let zm = prev[ib - dz as usize];
                // Move right face to left and accumulate
                acc[0] += zm[6];
                acc[1] += zm[7];
            }

            // Fixup missing data from Z+ block, which is needed to fill right the current
            // block when it's shifted left. Because a Z face decompresses to 2
            // u64, we actually read from the entire face directly, and
            // accumulate.
            if bpos.z + 1 < block_count.z {
                let zp = prev[ib + dz as usize];
                // Move left face to right and accumulate
                acc[6] += zp[0];
                acc[7] += zp[1];
            }

            // Copy counts into output array
            dst[ic..ic + 64].copy_from_slice(bytemuck::cast_slice(&acc[..]));
            ic += 64;

            // Update block position
            bpos.x += 1;
            if bpos.x >= block_count.x {
                bpos.x = 0;
                bpos.y += 1;
                if bpos.y >= block_count.y {
                    bpos.y = 0;
                    bpos.z += 1;
                }
            }
        }
    }

    /// Count Moore 8-neighbors (or, 26 in 3D) with a separable sum.
    pub(crate) fn count_neighbors_separable_m(&self, default: bool) -> Vec<u8> {
        let block_count = (self.size.as_ivec3() + 3) / 4;

        // Over-allocate entire blocks to avoid having to bound-check the writes
        let capacity =
            block_count.x as usize * block_count.y as usize * block_count.z as usize * 64;
        let mut counts = Vec::with_capacity(capacity);
        counts.resize(capacity, 0);
        let mut counts2 = counts.clone();

        // Separable sum over X then Y then Z
        Self::acc_x(&self.data[..], &mut counts[..], block_count);
        Self::acc_y(&counts, &mut counts2, block_count);
        Self::acc_z(&counts2, &mut counts, block_count);

        // Remove self, because we count only neighbors
        let mut ic = 0;
        for b in self.data.iter() {
            let b: [u8; 64] = bytemuck::cast(Self::decompress_block(*b));
            for i in 0..64 {
                counts[ic + i] -= b[i];
            }
            ic += 64;
        }

        counts
    }

    /// Count von Neumann 4-neighbors (or, 6 in 3D) with a separable sum.
    pub(crate) fn count_neighbors_separable_vn(&self, default: bool) -> Vec<u8> {
        let block_count = self.size.as_ivec3() / 4;

        // Over-allocate entire blocks to avoid having to bound-check the writes
        let capacity =
            block_count.x as usize * block_count.y as usize * block_count.z as usize * 64;
        let mut counts = Vec::with_capacity(capacity);
        counts.resize(capacity, 0);

        // Separable sum over X then Y then Z
        let mut bpos = IVec3::ZERO;
        let dy = block_count.x;
        let dz = block_count.x * block_count.y;
        let mut ic = 0;
        for (ib, b) in self.data.iter().enumerate() {
            // Shifted X
            let mut bxm = (b >> 1) & 0x7777_7777_7777_7777u64;
            let mut bxp = (b << 1) & 0xEEEE_EEEE_EEEE_EEEEu64;
            if bpos.x + 1 < block_count.x {
                // Move upper bit from next block
                let bp = (self.data[ib + 1] & 0x1111_1111_1111_1111u64) << 3;
                bxm |= bp;
            }
            if bpos.x > 0 {
                // Move lower bit from previous block
                let bm = (self.data[ib - 1] & 0x8888_8888_8888_8888u64) >> 3;
                bxp |= bm;
            }

            // Shifted Y
            let mut bym = (b >> 4) & 0x0FFF_0FFF_0FFF_0FFFu64;
            let mut byp = (b << 4) & 0xFFF0_FFF0_FFF0_FFF0u64;
            if bpos.y + 1 < block_count.y {
                // Move upper bit from next block
                let bp = (self.data[ib + dy as usize] & 0x000F_000F_000F_000Fu64) << 12;
                bym |= bp;
            }
            if bpos.y > 0 {
                // Move lower bit from previous block
                let bm = (self.data[ib - dy as usize] & 0xF000_F000_F000_F000u64) >> 12;
                byp |= bm;
            }

            // Shifted Z
            let mut bzm = (b >> 16) & 0x0FFF_0FFF_0FFF_0FFFu64;
            let mut bzp = (b << 16) & 0xFFF0_FFF0_FFF0_FFF0u64;
            if bpos.z + 1 < block_count.z {
                // Move upper bit from next block
                let bp = (self.data[ib + dz as usize] & 0x0000_0000_0000_FFFFu64) << 48;
                bzm |= bp;
            }
            if bpos.z > 0 {
                // Move lower bit from previous block
                let bm = (self.data[ib - dz as usize] & 0xFFFF_0000_0000_0000u64) >> 48;
                bzp |= bm;
            }

            // Accumulate
            let mut acc = [0u64; 8];
            for i in 0..8 {
                let shift = i as u64 * 8;
                let mask = 0xFFu64 << shift;
                acc[i] += Self::bit_to_byte((bxm & mask) >> shift);
                acc[i] += Self::bit_to_byte((bxp & mask) >> shift);
                acc[i] += Self::bit_to_byte((bym & mask) >> shift);
                acc[i] += Self::bit_to_byte((byp & mask) >> shift);
                acc[i] += Self::bit_to_byte((bzm & mask) >> shift);
                acc[i] += Self::bit_to_byte((bzp & mask) >> shift);

                for j in 0..8 {
                    counts[ic + 0] = (acc[i] & 0xFFu64) as u8;
                    counts[ic + 1] = ((acc[i] & 0xFF00u64) >> 8) as u8;
                    counts[ic + 2] = ((acc[i] & 0xFF_0000u64) >> 16) as u8;
                    counts[ic + 3] = ((acc[i] & 0xFF00_0000u64) >> 24) as u8;
                    counts[ic + 4] = ((acc[i] & 0xFF_0000_0000u64) >> 32) as u8;
                    counts[ic + 5] = ((acc[i] & 0xFF00_0000_0000u64) >> 40) as u8;
                    counts[ic + 6] = ((acc[i] & 0xFF_0000_0000_0000u64) >> 48) as u8;
                    counts[ic + 7] = ((acc[i] & 0xFF00_0000_0000_0000u64) >> 56) as u8;
                }
                ic += 8;
            }

            bpos.x += 1;
            if bpos.x >= block_count.x {
                bpos.x = 0;
                bpos.y += 1;
                if bpos.y >= block_count.y {
                    bpos.y = 0;
                    bpos.z += 1;
                }
            }
        }

        counts
    }

    fn count_neighbors_ref(&self, default: bool) -> Vec<u8> {
        let capacity = self.size.x as usize * self.size.y as usize * self.size.z as usize;
        let mut counts = Vec::with_capacity(capacity);
        counts.resize(capacity, 0);

        let mut pos = IVec3::ZERO;
        for k in [0, self.size.z as i32 - 1] {
            pos.z = k;
            let index = k * self.size.y as i32 * self.size.x as i32;
            for j in 0..self.size.y as i32 {
                pos.y = j as i32;
                let index = index + j * self.size.x as i32;
                for i in 0..self.size.x as i32 {
                    pos.x = i as i32;
                    let index = index + i;
                    counts[index as usize] = self.count_neighbors_single(pos, default);
                }
            }
        }

        counts
    }

    /// Reference single-threaded implementation of [`count_neighbors()`]. Very
    /// slow.
    fn count_neighbors_single(&self, pos: IVec3, default: bool) -> u8 {
        let mut count = 0;
        let mut xyz = pos;
        for k in (pos.z - 1)..=(pos.z + 1) {
            xyz.z = k;
            for j in (pos.y - 1)..=(pos.y + 1) {
                xyz.y = j;
                for i in (pos.x - 1)..=(pos.x + 1) {
                    xyz.x = i;
                    if xyz != pos && self.cell(xyz).unwrap_or(default) {
                        count += 1;
                    }
                }
            }
        }
        count
    }
}

fn fill_rand(capacity: usize, fill_ratio: f32, mut prng: impl RngCore) -> Vec<u64> {
    let mut data = Vec::with_capacity(capacity);
    for _ in 0..capacity {
        let mut v = 0u64;
        for b in 0..64 {
            let p: f32 = prng.gen_range(0.0..=1.0);
            let v0 = (p < fill_ratio) as u64;
            v |= v0 << b;
        }
        data.push(v);
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rulebitset2() {
        let r0 = RuleBitset2::from(0x01F0u16);
        assert_eq!(r0, RuleBitset2::from_bits(0x01F0u16));
        assert_eq!(r0.to_bits(), 0x01F0u16);
        assert_eq!(r0.to_array()[0..4], [false; 4]);
        assert_eq!(r0.to_array()[4..=8], [true; 5]);
        assert_eq!(RuleBitset2::from(r0.to_array()), r0);

        let r1 = RuleBitset2::from(1u8..4u8);
        assert_eq!(r1, RuleBitset2::from_bits(0xEu16));
        assert_eq!(r1.to_bits(), 0xEu16);
        assert_eq!(r1.to_array()[0], false);
        assert_eq!(r1.to_array()[1..4], [true; 3]);
        assert_eq!(r1.to_array()[4..], [false; 5]);
        assert_eq!(RuleBitset2::from(r1.to_array()), r1);

        let r2 = RuleBitset2::from(2u8..=7u8);
        assert_eq!(r2, RuleBitset2::from_bits(0xFCu16));
        assert_eq!(r2.to_bits(), 0xFCu16);
        assert_eq!(r2.to_array()[0..2], [false; 2]);
        assert_eq!(r2.to_array()[2..=7], [true; 6]);
        assert_eq!(r2.to_array()[8], false);
        assert_eq!(RuleBitset2::from(r2.to_array()), r2);

        let r12 = r1 | r2;
        assert_eq!(r12, RuleBitset2::from_bits(0xFEu16));
        assert_eq!(r12.to_bits(), 0xFEu16);
        assert_eq!(r12.to_array()[0], false);
        assert_eq!(r12.to_array()[1..=7], [true; 7]);
        assert_eq!(r12.to_array()[8], false);
        assert_eq!(RuleBitset2::from(r12.to_array()), r12);
    }

    #[test]
    #[should_panic]
    fn rulebitset2_frombits_invalid() {
        let _r = RuleBitset2::from_bits(0xFFFFu16);
    }

    #[test]
    fn rule2_smooth() {
        // S5-8/B4-8/2/M
        let rule = Rule2 {
            birth: (5u8..=8u8).into(),
            survive: (4u8..=8u8).into(),
        };
        assert_eq!(Rule2::SMOOTH, rule);
    }

    #[test]
    fn rulebitset3() {
        let r0 = RuleBitset3::from(0xFF00u32);
        assert_eq!(r0, RuleBitset3::from_bits(0xFF00u32));
        assert_eq!(r0.to_bits(), 0xFF00u32);
        assert_eq!(r0.to_array()[0..8], [false; 8]);
        assert_eq!(r0.to_array()[8..16], [true; 8]);
        assert_eq!(r0.to_array()[16..], [false; 11]);
        assert_eq!(RuleBitset3::from(r0.to_array()), r0);

        let r1 = RuleBitset3::from(3u8..7u8);
        assert_eq!(r1, RuleBitset3::from_bits(0x78u32));
        assert_eq!(r1.to_bits(), 0x78u32);
        assert_eq!(r1.to_array()[0..3], [false; 3]);
        assert_eq!(r1.to_array()[3..7], [true; 4]);
        assert_eq!(r1.to_array()[7..], [false; 20]);
        assert_eq!(RuleBitset3::from(r1.to_array()), r1);

        let r2 = RuleBitset3::from(23u8..=26u8);
        assert_eq!(r2, RuleBitset3::from_bits(0x780_0000u32));
        assert_eq!(r2.to_bits(), 0x780_0000u32);
        assert_eq!(r2.to_array()[0..23], [false; 23]);
        assert_eq!(r2.to_array()[23..], [true; 4]);
        assert_eq!(RuleBitset3::from(r2.to_array()), r2);

        let r12 = r1 | r2;
        assert_eq!(r12, RuleBitset3::from_bits(0x780_0078u32));
        assert_eq!(r12.to_bits(), 0x780_0078u32);
        assert_eq!(r12.to_array()[0..3], [false; 3]);
        assert_eq!(r12.to_array()[3..7], [true; 4]);
        assert_eq!(r12.to_array()[7..23], [false; 16]);
        assert_eq!(r12.to_array()[23..], [true; 4]);
        assert_eq!(RuleBitset3::from(r12.to_array()), r12);
    }

    #[test]
    #[should_panic]
    fn rulebitset3_frombits_invalid() {
        let _r = RuleBitset3::from_bits(0xFFFF_FFFFu32);
    }

    #[test]
    fn rule3_smooth() {
        // 13-26/13-14,17-19/2/M
        let rule = Rule3 {
            birth: RuleBitset3::from(13u8..=14u8) | (17u8..=19u8).into(),
            survive: (13u8..=26u8).into(),
        };
        assert_eq!(Rule3::SMOOTH, rule);
    }

    fn index(pos: IVec3, size: UVec3) -> usize {
        let index = pos.z as u32 * size.y * size.x + pos.y as u32 * size.x + pos.x as u32;
        index as usize
    }

    #[test]
    fn bit_to_byte() {
        assert_eq!(Grid3::bit_to_byte(0), 0);
        assert_eq!(Grid3::bit_to_byte(1), 1);
        assert_eq!(Grid3::bit_to_byte(0b00_01_00_11), 0x0000_0001_0000_0101u64);
        assert_eq!(Grid3::bit_to_byte(0b10_00_00_01), 0x0100_0000_0000_0001u64);
        assert_eq!(Grid3::bit_to_byte(0b11_11_11_11), 0x0101_0101_0101_0101u64);
    }

    #[test]
    fn resolve() {
        let size = UVec3::ONE * 8;
        let grid = Grid3::new(size);
        assert_eq!(grid.resolve_bit(IVec3::ZERO), Some((0, 1u64 << 0)));
        assert_eq!(grid.resolve_bit(IVec3::ONE), Some((0, 1u64 << 21)));
        assert_eq!(grid.resolve_bit(IVec3::X * 4), Some((1, 1u64 << 0)));
        assert_eq!(grid.resolve_bit(IVec3::ONE * 4), Some((7, 1u64 << 0)));
        assert_eq!(grid.resolve_bit(IVec3::ONE * 7), Some((7, 1u64 << 63)));
    }

    #[test]
    fn count_neighbors_separable() {
        // 8x8x8 grid (2x2x2 blocks)
        let size = UVec3::ONE * 8;
        let mut grid = Grid3::new(size);
        grid.fill(false);

        // Grid corner
        {
            let mut grid = grid.clone();

            // Set only 1 cell #0 at (0,0,0)
            grid.set_cell(IVec3::ZERO, true);

            // Check counts
            let counts = grid.count_neighbors_separable_m(false);
            // All 26 Moore neighbors around (0,0,0)
            for i in [
                // ------------------------------------------------------------- Z = 0
                // Note: 0 == (0,0,0), we don't count self in neighbors
                /* 0, */
                1, 4, 5,
                // ------------------------------------------------------------- Z = 1
                16, 17, 20, 21,
            ] {
                assert_eq!(counts[i], 1);
            }
            for i in [0, 2, 3, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 18, 19] {
                assert_eq!(counts[i], 0);
            }
            // All other blocks except the first are empty
            for i in 22..512 {
                assert_eq!(counts[i], 0);
            }
        }

        // Inside 1 block, no spilling over other blocks
        {
            let mut grid = grid.clone();

            // Set only 1 cell #21 at (1,1,1)
            grid.set_cell(IVec3::ONE, true);

            // Check counts
            let counts = grid.count_neighbors_separable_m(false);
            // All 26 Moore neighbors around (1,1,1)
            for i in [
                // ------------------------------------- Z = 0
                0, 1, 2, 4, 5, 6, 8, 9, 10,
                // ------------------------------------- Z = 1
                // Note: 21 == (1,1,1), we don't count self in neighbors
                16, 17, 18, 20, /* 21, */ 22, 24, 25, 26,
                // ------------------------------------- Z = 2
                32, 33, 34, 36, 37, 38, 40, 41, 42,
            ] {
                assert_eq!(counts[i], 1);
            }
            for i in [
                3, 7, 11, 12, 13, 14, 15, 19, 21, 23, 27, 28, 29, 30, 31, 35, 39,
            ] {
                assert_eq!(counts[i], 0);
            }
            // All other blocks except the first are empty
            for i in 43..512 {
                assert_eq!(counts[i], 0);
            }
        }

        // Last bit of 1 block, spilling over all 7 other neighbor blocks
        {
            let mut grid = grid.clone();

            // Set only 1 cell #63 at (3,3,3)
            grid.set_cell(IVec3::ONE * 3, true);

            // Check counts
            let counts = grid.count_neighbors_separable_m(false);
            // All 26 Moore neighbors around (3,3,3)
            let cells = [
                // ------------------------------------- Z = 2
                42, 43, 46, 47,
                // ------------------------------------- Z = 3
                // Note: 63 == (3,3,3), we don't count self in neighbors
                58, 59, 62, /* 63 */
                // ------------------------------------- Second block, on X+ side: X=0, Y=3:4,
                // Z=3:4
                104, 108, 120, 124,
                // ------------------------------------- Third block, on Y+ side: X=3:4, Y=0, Z=3:4
                162, 163, 178, 179,
                // ------------------------------------- Fourth block, on X+Y+ side: X=Y=0, Z=3:4
                224, 240,
                // ------------------------------------- Fifth block, on Z+ side: X=3:4 Y=3:4, Z=0
                266, 267, 270, 271,
                // ------------------------------------- Sixth block, on X+Z+ side: X=0 Y=3:4, Z=0
                328, 332,
                // ------------------------------------- Seventh block, on Y+Z+ side: X=3:4 Y=0,
                // Z=0
                386, 387,
                // ------------------------------------- Eighth block, on X+Y+Z+ side: X=Y=Z=0
                448,
            ];
            for i in &cells {
                assert_eq!(counts[*i], 1);
            }
            for i in 0..448 {
                assert_eq!(counts[i], cells.contains(&i) as u8);
            }
            for i in 449..512 {
                assert_eq!(counts[i], 0);
            }
        }
    }

    #[test]
    fn neighbors3() {
        // 3x3x3 grid with alive cell in center
        let size = UVec3::ONE * 3;
        let mut grid = Grid3::new(size);
        grid.fill(false);
        grid.set_cell(IVec3::ONE, true);

        let counts_false = grid.count_neighbors(false);
        let counts_true = grid.count_neighbors(true);

        for k in -1..=1 {
            for j in -1..=1 {
                for i in -1..=1 {
                    let pos = IVec3::new(i + 1, j + 1, k + 1);
                    if pos == IVec3::ONE {
                        // center: no neighbor
                        assert_eq!(counts_false[index(pos, size)], 0);
                    } else if i * j * k != 0 {
                        // corner: neighbor is center, and optionally out-of-bound values if
                        // default=true
                        assert_eq!(counts_false[index(pos, size)], 1);
                        assert_eq!(counts_true[index(pos, size)], 20);
                    } else if i * j != 0 || i * k != 0 || j * k != 0 {
                        // edge center: neighbor is center, and optionally out-of-bound values if
                        // default=true
                        assert_eq!(counts_false[index(pos, size)], 1);
                        assert_eq!(counts_true[index(pos, size)], 16);
                    } else {
                        // face center: neighbor is center, and optionally out-of-bound values if
                        // default=true
                        assert_eq!(counts_false[index(pos, size)], 1);
                        assert_eq!(counts_true[index(pos, size)], 10);
                    }
                }
            }
        }
    }

    #[test]
    fn smooth3_4x4x4() {
        // 4x4x4 full grid
        let size = UVec3::ONE * 4;
        let mut grid = Grid3::new(size);

        grid.fill(true);
        assert_eq!(grid.data.len(), 1);
        assert_eq!(grid.data[0], !0u64);

        // Smoothing will shave off the edges
        grid.apply_rule(&Rule3::SMOOTH);
        for k in 0..4 {
            for j in 0..4 {
                for i in 0..4 {
                    let value = grid.cell(IVec3::new(i as i32, j as i32, k as i32)).unwrap();
                    let x_border = i == 0 || i == 3;
                    let y_border = j == 0 || j == 3;
                    let z_border = k == 0 || k == 3;
                    if (x_border && y_border) || (y_border && z_border) || (z_border && x_border) {
                        assert!(!value);
                    } else {
                        assert!(value);
                    }
                }
            }
        }
    }

    #[test]
    fn smooth3_8x8x8() {
        // 8x8x8 full grid
        let size = UVec3::ONE * 8;
        let mut grid = Grid3::new(size);

        grid.fill(true);
        assert_eq!(grid.data.len(), 8);
        for i in 0..8 {
            assert_eq!(grid.data[i], !0u64);
        }

        // Smoothing will shave off the edges
        grid.apply_rule(&Rule3::SMOOTH);
        for k in 0..8 {
            for j in 0..8 {
                for i in 0..8 {
                    let value = grid.cell(IVec3::new(i as i32, j as i32, k as i32)).unwrap();
                    let x_border = i == 0 || i == 7;
                    let y_border = j == 0 || j == 7;
                    let z_border = k == 0 || k == 7;
                    if (x_border && y_border) || (y_border && z_border) || (z_border && x_border) {
                        if value {
                            println!("{} {} {}", i, j, k);
                        }
                        assert!(!value);
                    } else {
                        assert!(value);
                    }
                }
            }
        }
    }
}
