#![feature(portable_simd)]
#![feature(test)]
#![feature(stdarch_x86_avx512)]
// Enable Rust's built-in benchmarking (nightly required)
extern crate test; // Import Rust's benchmarking module

use rayon::prelude::*;
use std::arch::x86_64::{__m128i, __m256i, _mm256_cmpeq_epi8, _mm256_load_si256, _mm256_loadu_si256, _mm256_movemask_epi8, _mm_cmpeq_epi8, _mm_load_si128, _mm_loadu_si128, _mm_movemask_epi8};
use std::ops::Add;
use std::simd::cmp::SimdPartialEq;
use std::simd::Simd;

/// Compares two 16-byte slices using a 16-lane SIMD vector.
pub fn bytes_cmp_simd_16(a: &[u8], b: &[u8]) -> bool {
    // Ensure the slices have exactly 16 elements.
    debug_assert_eq!(a.len(), 16);
    debug_assert_eq!(b.len(), 16);
    let simd1 = Simd::<u8, 16>::from_slice(a);
    let simd2 = Simd::<u8, 16>::from_slice(b);
    simd1.simd_eq(simd2).all()
}

/// Compares two 32-byte slices using a 32-lane SIMD vector.
pub fn bytes_cmp_simd_32(a: &[u8], b: &[u8]) -> bool {
    debug_assert_eq!(a.len(), 32);
    debug_assert_eq!(b.len(), 32);
    let simd1 = Simd::<u8, 32>::from_slice(a);
    let simd2 = Simd::<u8, 32>::from_slice(b);
    simd1.simd_eq(simd2).all()
}

/// Compares two 64-byte slices using a 64-lane SIMD vector.
pub fn bytes_cmp_simd_64(a: &[u8], b: &[u8]) -> bool {
    debug_assert_eq!(a.len(), 64);
    debug_assert_eq!(b.len(), 64);
    let simd1 = Simd::<u8, 64>::from_slice(a);
    let simd2 = Simd::<u8, 64>::from_slice(b);
    simd1.simd_eq(simd2).all()
}

/// Compare two byte slices in 16-byte chunks.
fn compare_bytes_simd_16(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut iter_a = a.chunks_exact(16);
    let mut iter_b = b.chunks_exact(16);
    for (chunk_a, chunk_b) in iter_a.by_ref().zip(iter_b.by_ref()) {
        // SAFETY: chunk_a and chunk_b are exactly 16 bytes.
        let arr_a = unsafe { &*(chunk_a.as_ptr() as *const [u8; 16]) };
        let arr_b = unsafe { &*(chunk_b.as_ptr() as *const [u8; 16]) };
        if !bytes_cmp_simd_16(arr_a, arr_b) {
            return false;
        }
    }
    // Handle any remaining bytes.
    let remainder_a = iter_a.remainder();
    let remainder_b = iter_b.remainder();
    if !remainder_a.is_empty() {
        let mut padded_a = [0u8; 16];
        let mut padded_b = [0u8; 16];
        padded_a[..remainder_a.len()].copy_from_slice(remainder_a);
        padded_b[..remainder_b.len()].copy_from_slice(remainder_b);
        if !bytes_cmp_simd_16(&padded_a, &padded_b) {
            return false;
        }
    }
    true
}

/// Compare two byte slices in 32-byte chunks.
fn compare_bytes_simd_32(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut iter_a = a.chunks_exact(32);
    let mut iter_b = b.chunks_exact(32);
    for (chunk_a, chunk_b) in iter_a.by_ref().zip(iter_b.by_ref()) {
        // SAFETY: chunk_a and chunk_b are exactly 32 bytes.
        let arr_a = unsafe { &*(chunk_a.as_ptr() as *const [u8; 32]) };
        let arr_b = unsafe { &*(chunk_b.as_ptr() as *const [u8; 32]) };
        if !bytes_cmp_simd_32(arr_a, arr_b) {
            return false;
        }
    }
    // Handle any remaining bytes.
    let remainder_a = iter_a.remainder();
    let remainder_b = iter_b.remainder();
    if !remainder_a.is_empty() {
        if !remainder_a.is_empty() {
            if remainder_a != remainder_b {
                return false;
            }
        }
    }
    true
}

/// Compare slices in 64-byte chunks.
fn compare_bytes_simd_64(a: &[u8], b: &[u8]) -> bool {
    let mut iter_a = a.chunks_exact(64);
    let mut iter_b = b.chunks_exact(64);
    for (chunk_a, chunk_b) in iter_a.by_ref().zip(iter_b.by_ref()) {
        // SAFETY: chunk_a and chunk_b are exactly 64 bytes.
        let arr_a = unsafe { &*(chunk_a.as_ptr() as *const [u8; 64]) };
        let arr_b = unsafe { &*(chunk_b.as_ptr() as *const [u8; 64]) };
        if !bytes_cmp_simd_64(arr_a, arr_b) {
            return false;
        }
    }
    // Handle remainder.
    let remainder_a = iter_a.remainder();
    let remainder_b = iter_b.remainder();
    if !remainder_a.is_empty() {
        if remainder_a != remainder_b {
            return false;
        }
    }
    true
}

/// # Safety
/// Idk what to tell u man this shit is safe trust
use std::arch::x86_64::*;

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn compare_bytes_simd_avx2_256(a: &[u8], b: &[u8]) -> bool {
    debug_assert_eq!(a.len(), b.len());

    let len = a.len();
    let mut i = 0;

    while i + 32 <= len {
        let a_chunk = _mm256_loadu_si256(a.as_ptr().add(i) as *const __m256i);
        let b_chunk = _mm256_loadu_si256(b.as_ptr().add(i) as *const __m256i);

        let cmp = _mm256_cmpeq_epi8(a_chunk, b_chunk);
        let mask = _mm256_movemask_epi8(cmp);

        if mask != 0xFFFFFFFFu32 as i32 {
            return false; // Some bytes do not match
        }

        i += 32;
    }

    // Fallback for remaining bytes
    if a[i..] != b[i..] {
        return false;
    }

    true
}

unsafe fn compare_bytes_simd_avx512f(a: &[u8], b: &[u8]) -> bool {
    #[target_feature(enable = "avx512f")]
    let len = a.len();
    let mut i = 0;

    while i + 64 <= len {
        let a_chunk = _mm512_loadu_si512(a.as_ptr().add(i) as *const i32);
        let b_chunk = _mm512_loadu_si512(b.as_ptr().add(i) as *const i32);

        let mask = _mm512_cmpeq_epi8_mask(a_chunk, b_chunk);

        if mask != u64::MAX {
            return false;
        }

        i += 64;
    }
    a[i..] == b[i..]
}


fn compare_bytes_simd_dynamic(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let len = a.len();
    if len < 16 {
        a == b
    } else if len < 32 {
        // Use SIMD in 16-byte chunks.
        compare_bytes_simd_16(a, b)
    } else if len < 64 {
        // Use SIMD in 32-byte chunks.
        compare_bytes_simd_32(a, b)
    } else {
        // Use SIMD in 64-byte chunks.
        compare_bytes_simd_64(a, b)
    }
}

unsafe fn compare_bytes_intrinsics_dynamic(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    if is_x86_feature_detected!("avx2") {
        compare_bytes_simd_avx2_256(a, b)
    }
    else if is_x86_feature_detected!("avx512f") {
        compare_bytes_simd_avx512f(a, b)
    }
    else {
        unimplemented!()
    }
}

pub fn compare_string_vectors_simd_non_par(
    haystack1: &[String],
    haystack2: &[String],
) -> Vec<(usize, usize)> {
    haystack1
        .iter()
        .enumerate()
        .flat_map(|(i, s1)| {
            let bytearray1 = s1.as_bytes();
            haystack2
                .iter()
                .enumerate()
                .filter_map(move |(j, s2)| unsafe {
                    let bytearray2 = s2.as_bytes();
                    if bytearray1.len() != bytearray2.len() {
                        return None;
                    }
                    if compare_bytes_intrinsics_dynamic(bytearray1, bytearray2) {
                        Some((i, j))
                    } else {
                        None
                    }
                })
        })
        .collect()
}


pub fn compare_string_vectors_simd(
    haystack1: &[String],
    haystack2: &[String],
) -> Vec<(usize, usize)> {
    haystack1
        .par_iter()
        .enumerate()
        .flat_map(|(i, s1)| {
            let bytearray1 = s1.as_bytes();
            haystack2
                .par_iter()
                .enumerate()
                .filter_map(move |(j, s2)| unsafe {
                    let bytearray2 = s2.as_bytes();
                    if bytearray1.len() != bytearray2.len() {
                        return None;
                    }
                    if compare_bytes_intrinsics_dynamic(bytearray1, bytearray2) {
                        Some((i, j))
                    } else {
                        None
                    }
                })
        })
        .collect()
}

pub fn compare_string_vectors(haystack1: &[String], haystack2: &[String]) -> Vec<(usize, usize)> {
    haystack1
        .par_iter()
        .enumerate()
        .flat_map(|(i, s1)| {
            let bytearray1 = s1.as_bytes();
            haystack2.par_iter().enumerate().filter_map(move |(j, s2)| {
                let bytearray2 = s2.as_bytes();
                if bytearray1.len() != bytearray2.len() {
                    return None;
                }
                if compare_bytes_simd_dynamic(bytearray1, bytearray2) {
                    Some((i, j))
                } else {
                    None
                }
            })
        })
        .collect()
}

pub fn compare_string_vectors_naive(
    haystack1: &[String],
    haystack2: &[String],
) -> Vec<(usize, usize)> {
    let mut conflicts = Vec::new();
    for (i, s1) in haystack1.iter().enumerate() {
        for (j, s2) in haystack2.iter().enumerate() {
            if s1 == s2 {
                // Simple comparison, no chunking or SIMD
                conflicts.push((i, j));
            }
        }
    }
    conflicts
}

#[cfg(test)]
#[cfg(target_arch = "x86_64")]
mod tests {
    use super::*;
    // Test: two identical strings in the vectors should be reported as a conflict.
    #[test]
    fn test_identical_strings() {
        let haystack1 = vec!["hello".to_string(), "world".to_string()];
        let haystack2 = vec!["hello".to_string(), "rust".to_string()];
        let conflicts = compare_string_vectors_simd(&haystack1, &haystack2);
        // "hello" vs "hello" should be flagged as a conflict.
        assert_eq!(conflicts, vec![(0, 0)]);


        assert_eq!(compare_string_vectors(&haystack1, &haystack2),
                   compare_string_vectors_simd(&haystack1,&haystack2));
    }

    // Test: one string being a prefix of the other is considered a conflict.
    #[test]
    fn test_prefix_conflict() {
        let haystack1 = vec!["he".to_string()];
        let haystack2 = vec!["hello".to_string()];
        let conflicts = compare_string_vectors_simd(&haystack1, &haystack2);
        assert!(conflicts.is_empty());


        assert_eq!(compare_string_vectors(&haystack1, &haystack2),
                   compare_string_vectors_simd(&haystack1,&haystack2));
    }

    // Test: strings that do not match should not produce any conflict.
    #[test]
    fn test_no_conflict() {
        let haystack1 = vec!["hello".to_string()];
        let haystack2 = vec!["worl".to_string()];
        let conflicts = compare_string_vectors_simd(&haystack1, &haystack2);
        assert!(conflicts.is_empty());


        assert_eq!(compare_string_vectors(&haystack1, &haystack2),
                   compare_string_vectors_simd(&haystack1,&haystack2));
    }

    // Test: how empty strings are handled.
    #[test]
    fn test_empty_strings() {
        let haystack1 = vec!["".to_string(), "nonempty".to_string()];
        let haystack2 = vec!["anything".to_string(), "".to_string()];
        let conflicts = compare_string_vectors_simd(&haystack1, &haystack2);
        // For the empty string cases:
        // - "" (haystack1[0]) vs "anything" (haystack2[0]): len=0 so the so conflict is not detected.
        // - "nonempty" (haystack1[1]) vs "" (haystack2[1]): len=0, so conflict is not detected.
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts.contains(&(0, 1)));


        assert_eq!(compare_string_vectors(&haystack1, &haystack2),
                   compare_string_vectors_simd(&haystack1,&haystack2));
    }

    // Test: multiple strings with some conflicts.
    #[test]
    fn test_multiple_conflicts() {
        let haystack1 = vec![
            "foobar".to_string(),
            "barbecue".to_string(),
            "baz".to_string(),
        ];
        let haystack2 = vec![
            "foobar".to_string(),
            "barbecue".to_string(),
            "baz".to_string(),
        ];
        let conflicts = compare_string_vectors_simd(&haystack1, &haystack2);
        // Analysis:
        // - "foo" (haystack1[0]) vs "foobar" (haystack2[0]): min length = 3, "foo" == "foo", conflict.
        // - "bar" (haystack1[1]) vs "barbecue" (haystack2[1]): min length = 3, "bar" == "bar", conflict.
        // Other comparisons do not result in conflicts.
        let mut conflicts_sorted = conflicts.clone();
        conflicts_sorted.sort();
        let mut expected = vec![(0, 0), (1, 1), (2, 2)];
        expected.sort();


        assert_eq!(compare_string_vectors(&haystack1, &haystack2),
                   compare_string_vectors_simd(&haystack1,&haystack2));
        assert_eq!(conflicts_sorted, expected);


        assert_eq!(compare_string_vectors(&haystack1, &haystack2),
                   compare_string_vectors_simd(&haystack1,&haystack2));
    }

    #[test]
    fn test_really_long_strings_with_same_prefix() {
        // Create two strings of length 1024.
        // They share the same prefix ("a" repeated 512 times),
        // but then they differ by one character before sharing the remainder.
        let s1 = "a".repeat(1000);
        let s2 = format!("{}{}{}", "a".repeat(499), "b", "a".repeat(500));
        // Ensure both strings have the same length.
        assert_eq!(s1.len(), s2.len());

        let haystack1 = vec![s1];
        let haystack2 = vec![s2];

        // Since the strings differ by one character, no conflict should be detected.
        let conflicts = compare_string_vectors_simd(&haystack1, &haystack2);
        assert!(
            conflicts.is_empty(),
            "Expected no conflict because the strings differ after the common prefix"
        );


        assert_eq!(compare_string_vectors(&haystack1, &haystack2),
                   compare_string_vectors_simd(&haystack1,&haystack2));
    }

    #[test]
    fn test_really_long_strings() {
        // Create two strings of length 1024.
        // They share the same prefix ("a" repeated 512 times),
        // but then they differ by one character before sharing the remainder.
        let s1 = "a".repeat(1000);

        let haystack1 = vec![s1.clone()];
        let haystack2 = vec![s1.clone()];

        // Since the strings differ by one character, no conflict should be detected.
        let conflicts = compare_string_vectors(&haystack1, &haystack2);
        assert_eq!(conflicts.len(), 1);


        assert_eq!(compare_string_vectors(&haystack1, &haystack2),
                   compare_string_vectors_simd(&haystack1,&haystack2));
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "x86_64"))]
mod tests {
    use super::*;
    // Test: two identical strings in the vectors should be reported as a conflict.
    #[test]
    fn test_identical_strings() {
        let haystack1 = vec!["hello".to_string(), "world".to_string()];
        let haystack2 = vec!["hello".to_string(), "rust".to_string()];
        let conflicts = compare_string_vectors(&haystack1, &haystack2);
        // "hello" vs "hello" should be flagged as a conflict.
        assert_eq!(conflicts, vec![(0, 0)]);
    }

    // Test: one string being a prefix of the other is considered a conflict.
    #[test]
    fn test_prefix_conflict() {
        let haystack1 = vec!["he".to_string()];
        let haystack2 = vec!["hello".to_string()];
        let conflicts = compare_string_vectors(&haystack1, &haystack2);
        assert!(conflicts.is_empty());
    }

    // Test: strings that do not match should not produce any conflict.
    #[test]
    fn test_no_conflict() {
        let haystack1 = vec!["hello".to_string()];
        let haystack2 = vec!["worl".to_string()];
        let conflicts = compare_string_vectors(&haystack1, &haystack2);
        assert!(conflicts.is_empty());
    }

    // Test: how empty strings are handled.
    #[test]
    fn test_empty_strings() {
        let haystack1 = vec!["".to_string(), "nonempty".to_string()];
        let haystack2 = vec!["anything".to_string(), "".to_string()];
        let conflicts = compare_string_vectors(&haystack1, &haystack2);
        // For the empty string cases:
        // - "" (haystack1[0]) vs "anything" (haystack2[0]): len=0 so the so conflict is not detected.
        // - "nonempty" (haystack1[1]) vs "" (haystack2[1]): len=0, so conflict is not detected.
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts.contains(&(0, 1)));
    }

    // Test: multiple strings with some conflicts.
    #[test]
    fn test_multiple_conflicts() {
        let haystack1 = vec![
            "foobar".to_string(),
            "barbecue".to_string(),
            "baz".to_string(),
        ];
        let haystack2 = vec![
            "foobar".to_string(),
            "barbecue".to_string(),
            "baz".to_string(),
        ];
        let conflicts = compare_string_vectors(&haystack1, &haystack2);
        // Analysis:
        // - "foo" (haystack1[0]) vs "foobar" (haystack2[0]): min length = 3, "foo" == "foo", conflict.
        // - "bar" (haystack1[1]) vs "barbecue" (haystack2[1]): min length = 3, "bar" == "bar", conflict.
        // Other comparisons do not result in conflicts.
        let mut conflicts_sorted = conflicts.clone();
        conflicts_sorted.sort();
        let mut expected = vec![(0, 0), (1, 1), (2, 2)];
        expected.sort();
        assert_eq!(conflicts_sorted, expected);
    }

    #[test]
    fn test_really_long_strings_with_same_prefix() {
        // Create two strings of length 1024.
        // They share the same prefix ("a" repeated 512 times),
        // but then they differ by one character before sharing the remainder.
        let s1 = "a".repeat(1024);
        let s2 = format!("{}{}{}", "a".repeat(512), "b", "a".repeat(511));
        // Ensure both strings have the same length.
        assert_eq!(s1.len(), s2.len());

        let haystack1 = vec![s1];
        let haystack2 = vec![s2];

        // Since the strings differ by one character, no conflict should be detected.
        let conflicts = compare_string_vectors(&haystack1, &haystack2);
        assert!(
            conflicts.is_empty(),
            "Expected no conflict because the strings differ after the common prefix"
        );
    }
}
