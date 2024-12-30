#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

use glam::UVec3;

#[inline]
unsafe fn expand_b2b_32(
    bits: __m256i,
    mask_b2b_load: __m256i,
    mask_b2b_mask: __m256i,
    mask_bit1: __m256i,
) -> __m256i {
    // Copy 8 times into 32x8. The first 8 bytes contain a copy of the first byte,
    // the next 8 ones a copy of the second byte, etc. until the fourth byte
    // copied into bytes [24:31].
    let a = _mm256_shuffle_epi8(bits, mask_b2b_load);
    // Mask out all but the n-th bit of each of the 32 bytes. This means the first 8
    // bytes contain the first 8 bits (one per byte), and same for the next 3
    // chunks.
    let a = _mm256_and_si256(a, mask_b2b_mask);
    // Each byte has 1 bit set at most, but it's not always the same one (it's the
    // n-th one). Convert to the lowest bit by comparing to zero and masking.
    // The compare produces 0 or 0xFF, the masking keeps 0x1.
    let a = _mm256_cmpeq_epi8(a, mask_b2b_mask);
    _mm256_and_si256(a, mask_bit1)
}

pub unsafe fn count_neighbors(size: UVec3, data: &[u64], default: bool) -> Vec<u8> {
    let capacity = size.x as usize * size.y as usize * size.z as usize;
    let mut counts = Vec::with_capacity(capacity);
    counts.resize(capacity, 0u8);

    // AVX2 has 16 registers of 256 bits.
    // -

    // Grab 4 blocks of 4x4x4 cells (64 bits) into one 256-bit register.
    {
        let src = data.as_ptr();
        let num_blocks = (size.x / 4) as usize;
        for iblock in 0..num_blocks {
            // Load 4 blocks
            let a = _mm256_lddqu_si256(src.add(iblock * 4) as *const __m256i);

            // Left: x--
            let a_xm = {
                // Clear the left face, that is top bit of each group of 4 bits
                let mask_x234 = 0x7777_7777_7777_7777i64;
                let mask_x234 = _mm256_set_epi64x(mask_x234, mask_x234, mask_x234, mask_x234);
                let b = _mm256_and_si256(a, mask_x234);
                // Move everything 1 bit left; this gives the correct result for the top 3 bits
                // along X. Note that we move within a block of 64 bits (epi64),
                // so bits cannot overflow into an adjacent block.
                let b = _mm256_slli_epi64::<1>(b);

                // For the bottom (rightmost) X bit, it comes from the next block, so things are
                // a bit more complicated. Grab the left face (top bit), and
                // shift it 3 bits right to move it to be the right face.
                let c = _mm256_andnot_si256(a, mask_x234);
                let c = _mm256_srli_epi64::<3>(c);
                // Now we need to move each left face to the next block. This would mean a shift
                // by 64 bits. Unfortunately, _mm256_slli_si256() despite the
                // name works on 2 lanes of 128 bits, so would block the move in the middle.
                // Instead, since we move by 64, we can use _mm256_permute4x64_epi64() to
                // simulate a shift.
                let c = _mm256_permute4x64_epi64::<0b10_01_00_00>(c);
                // Clear the last block, for which we don't have data loaded, and was unchanged
                // by previous op.
                let mask_bbb0 = _mm256_set_epi64x(-1i64, -1i64, -1i64, 0i64);
                let c = _mm256_and_si256(c, mask_bbb0);

                // Combine. This contains the proper value for all but the low bit of the last
                // block, which requires some data from the next block (not
                // loaded yet).
                _mm256_or_si256(b, c)
            };

            // Right: x++
            let a_xp = {
                // Clear the right face, that is bottom bit of each group of 4 bits
                let mask_x123 = 0x1111_1111_1111_1111i64;
                let mask_x123 = _mm256_set_epi64x(mask_x123, mask_x123, mask_x123, mask_x123);
                let b = _mm256_and_si256(a, mask_x123);
                // Move everything 1 bit right; this gives the correct result for the bottom 3
                // bits along X. Note that we move within a block of 64 bits
                // (epi64), so bits cannot overflow into an adjacent block.
                let b = _mm256_srli_epi64::<1>(b);

                // For the top (leftmost) X bit, it comes from the previous block, so things are
                // a bit more complicated. Grab the right face (bottom bit), and
                // shift it 3 bits left to move it to be the left face.
                let c = _mm256_andnot_si256(a, mask_x123);
                let c = _mm256_slli_epi64::<3>(c);
                // Now we need to move each right face to the previous block. This would mean a
                // shift by 64 bits. Unfortunately, _mm256_srli_si256() despite
                // the name works on 2 lanes of 128 bits, so would block the move in the middle.
                // Instead, since we move by 64, we can use _mm256_permute4x64_epi64() to
                // simulate a shift.
                let c = _mm256_permute4x64_epi64::<0b11_11_10_01>(c);
                // Clear the first block, for which we don't have data loaded, and was unchanged
                // by previous op.
                let mask_0bbb = _mm256_set_epi64x(0i64, -1i64, -1i64, -1i64);
                let c = _mm256_and_si256(c, mask_0bbb);

                // Combine. This contains the proper value for all but the high bit of the first
                // block, which requires some data from the previous block (not
                // loaded yet).
                _mm256_or_si256(b, c)
            };

            let mask_b2b_mask = _mm256_set1_epi64x(bytemuck::cast(0x8040201008040201u64));
            let mask_bit1 = _mm256_set1_epi8(1);

            // Accumulate. There's up to 26 neighbors in 3D, so we need 5 bits per cell. For
            // simplicity we use 8 bits. This means we need to convert one
            // 256-bit 4-block result into 8 registers of 256-bit (32 values of 8 bits).
            let mask_b2b_load = _mm256_set_epi64x(
                0x0303030303030303,
                0x0202020202020202,
                0x0101010101010101,
                0x0000000000000000,
            );
            // Expand 32 bits to 32 bytes (lowest bit set of each byte)
            let acc0 = expand_b2b_32(a, mask_b2b_load, mask_b2b_mask, mask_bit1);
            // Same for x-, and accumulate
            let a_xm0 = expand_b2b_32(a_xm, mask_b2b_load, mask_b2b_mask, mask_bit1);
            let acc0 = _mm256_adds_epu8(acc0, a_xm0);
            // Same for x+, and accumulate
            let a_xp0 = expand_b2b_32(a_xp, mask_b2b_load, mask_b2b_mask, mask_bit1);
            let acc0 = _mm256_adds_epu8(acc0, a_xp0);

            // Next, bits [32:63]
            let inc_mask = _mm256_set1_epi64x(0x0404040404040404);
            let mask_b2b_load = _mm256_add_epi8(mask_b2b_load, inc_mask);
            let acc1 = expand_b2b_32(a, mask_b2b_load, mask_b2b_mask, mask_bit1);
            // Same for x-, and accumulate
            let a_xm0 = expand_b2b_32(a_xm, mask_b2b_load, mask_b2b_mask, mask_bit1);
            let acc1 = _mm256_adds_epu8(acc1, a_xm0);
            // Same for x+, and accumulate
            let a_xp0 = expand_b2b_32(a_xp, mask_b2b_load, mask_b2b_mask, mask_bit1);
            let acc1 = _mm256_adds_epu8(acc1, a_xp0);
        }
    }

    counts
}
