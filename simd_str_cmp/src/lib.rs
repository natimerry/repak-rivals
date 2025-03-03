#![feature(portable_simd)]
#![feature(test)]  // Enable Rust's built-in benchmarking (nightly required)
extern crate test;  // Import Rust's benchmarking module



use std::simd::cmp::SimdPartialEq;
use std::simd::Simd;
use rayon::prelude::*;



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
        let mut padded_a = [0u8; 32];
        let mut padded_b = [0u8; 32];
        padded_a[..remainder_a.len()].copy_from_slice(remainder_a);
        padded_b[..remainder_b.len()].copy_from_slice(remainder_b);
        if !bytes_cmp_simd_32(&padded_a, &padded_b) {
            return false;
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
        let mut padded_a = [0u8; 64];
        let mut padded_b = [0u8; 64];
        padded_a[..remainder_a.len()].copy_from_slice(remainder_a);
        padded_b[..remainder_b.len()].copy_from_slice(remainder_b);
        if !bytes_cmp_simd_64(&padded_a, &padded_b) {
            return false;
        }
    }
    true
}

/// Compare slices in 128-byte chunks by splitting each chunk into two 64-byte halves.
fn compare_bytes_simd_128(a: &[u8], b: &[u8]) -> bool {
    let mut iter_a = a.chunks_exact(128);
    let mut iter_b = b.chunks_exact(128);
    for (chunk_a, chunk_b) in iter_a.by_ref().zip(iter_b.by_ref()) {
        let (a1, a2) = chunk_a.split_at(64);
        let (b1, b2) = chunk_b.split_at(64);
        let arr_a1 = unsafe { &*(a1.as_ptr() as *const [u8; 64]) };
        let arr_a2 = unsafe { &*(a2.as_ptr() as *const [u8; 64]) };
        let arr_b1 = unsafe { &*(b1.as_ptr() as *const [u8; 64]) };
        let arr_b2 = unsafe { &*(b2.as_ptr() as *const [u8; 64]) };
        if !bytes_cmp_simd_64(arr_a1, arr_b1) || !bytes_cmp_simd_64(arr_a2, arr_b2) {
            return false;
        }
    }
    let remainder_a = iter_a.remainder();
    let remainder_b = iter_b.remainder();
    if !remainder_a.is_empty() {
        let mut padded_a = [0u8; 128];
        let mut padded_b = [0u8; 128];
        padded_a[..remainder_a.len()].copy_from_slice(remainder_a);
        padded_b[..remainder_b.len()].copy_from_slice(remainder_b);
        let (a1, a2) = padded_a.split_at(64);
        let (b1, b2) = padded_b.split_at(64);
        let arr_a1 = unsafe { &*(a1.as_ptr() as *const [u8; 64]) };
        let arr_a2 = unsafe { &*(a2.as_ptr() as *const [u8; 64]) };
        let arr_b1 = unsafe { &*(b1.as_ptr() as *const [u8; 64]) };
        let arr_b2 = unsafe { &*(b2.as_ptr() as *const [u8; 64]) };
        if !bytes_cmp_simd_64(arr_a1, arr_b1) || !bytes_cmp_simd_64(arr_a2, arr_b2) {
            return false;
        }
    }
    true
}

/// Compare slices in 256-byte chunks by splitting each chunk into four 64-byte parts.
fn compare_bytes_simd_256(a: &[u8], b: &[u8]) -> bool {
    let mut iter_a = a.chunks_exact(256);
    let mut iter_b = b.chunks_exact(256);
    for (chunk_a, chunk_b) in iter_a.by_ref().zip(iter_b.by_ref()) {
        // Split the 256-byte chunk into four 64-byte parts.
        let (a1, rest_a) = chunk_a.split_at(64);
        let (a2, rest_a) = rest_a.split_at(64);
        let (a3, a4) = rest_a.split_at(64);
        let (b1, rest_b) = chunk_b.split_at(64);
        let (b2, rest_b) = rest_b.split_at(64);
        let (b3, b4) = rest_b.split_at(64);
        let arr_a1 = unsafe { &*(a1.as_ptr() as *const [u8; 64]) };
        let arr_a2 = unsafe { &*(a2.as_ptr() as *const [u8; 64]) };
        let arr_a3 = unsafe { &*(a3.as_ptr() as *const [u8; 64]) };
        let arr_a4 = unsafe { &*(a4.as_ptr() as *const [u8; 64]) };
        let arr_b1 = unsafe { &*(b1.as_ptr() as *const [u8; 64]) };
        let arr_b2 = unsafe { &*(b2.as_ptr() as *const [u8; 64]) };
        let arr_b3 = unsafe { &*(b3.as_ptr() as *const [u8; 64]) };
        let arr_b4 = unsafe { &*(b4.as_ptr() as *const [u8; 64]) };
        if !bytes_cmp_simd_64(arr_a1, arr_b1)
            || !bytes_cmp_simd_64(arr_a2, arr_b2)
            || !bytes_cmp_simd_64(arr_a3, arr_b3)
            || !bytes_cmp_simd_64(arr_a4, arr_b4)
        {
            return false;
        }
    }
    let remainder_a = iter_a.remainder();
    let remainder_b = iter_b.remainder();
    if !remainder_a.is_empty() {
        let mut padded_a = [0u8; 256];
        let mut padded_b = [0u8; 256];
        padded_a[..remainder_a.len()].copy_from_slice(remainder_a);
        padded_b[..remainder_b.len()].copy_from_slice(remainder_b);
        let (a1, rest_a) = padded_a.split_at(64);
        let (a2, rest_a) = rest_a.split_at(64);
        let (a3, a4) = rest_a.split_at(64);
        let (b1, rest_b) = padded_b.split_at(64);
        let (b2, rest_b) = rest_b.split_at(64);
        let (b3, b4) = rest_b.split_at(64);
        let arr_a1 = unsafe { &*(a1.as_ptr() as *const [u8; 64]) };
        let arr_a2 = unsafe { &*(a2.as_ptr() as *const [u8; 64]) };
        let arr_a3 = unsafe { &*(a3.as_ptr() as *const [u8; 64]) };
        let arr_a4 = unsafe { &*(a4.as_ptr() as *const [u8; 64]) };
        let arr_b1 = unsafe { &*(b1.as_ptr() as *const [u8; 64]) };
        let arr_b2 = unsafe { &*(b2.as_ptr() as *const [u8; 64]) };
        let arr_b3 = unsafe { &*(b3.as_ptr() as *const [u8; 64]) };
        let arr_b4 = unsafe { &*(b4.as_ptr() as *const [u8; 64]) };
        if !bytes_cmp_simd_64(arr_a1, arr_b1)
            || !bytes_cmp_simd_64(arr_a2, arr_b2)
            || !bytes_cmp_simd_64(arr_a3, arr_b3)
            || !bytes_cmp_simd_64(arr_a4, arr_b4)
        {
            return false;
        }
    }
    true
}

/// Dynamically determine an optimal chunk size based on the string length,
/// then dispatch to the appropriate comparison routine.
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
    } else if len < 128 {
        // Use SIMD in 64-byte chunks.
        compare_bytes_simd_64(a, b)
    } else if len < 256 {
        // Use SIMD in 128-byte chunks.
        compare_bytes_simd_128(a, b)
    } else {
        // For larger slices, use SIMD in 256-byte chunks.
        compare_bytes_simd_256(a, b)
    }
}


pub fn compare_string_vectors(haystack1: Vec<String>, haystack2: Vec<String>) -> Vec<(usize, usize)> {
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

pub fn compare_string_vectors_naive(haystack1: Vec<String>, haystack2: Vec<String>) -> Vec<(usize, usize)> {
    let mut conflicts = Vec::new();
    for (i, s1) in haystack1.iter().enumerate() {
        for (j, s2) in haystack2.iter().enumerate() {
            if s1 == s2 { // Simple comparison, no chunking or SIMD
                conflicts.push((i, j));
            }
        }
    }
    conflicts
}




#[cfg(test)]
mod tests {
    use super::*;
    // Test: two identical strings in the vectors should be reported as a conflict.
    #[test]
    fn test_identical_strings() {
        let haystack1 = vec!["hello".to_string(), "world".to_string()];
        let haystack2 = vec!["hello".to_string(), "rust".to_string()];
        let conflicts = compare_string_vectors(haystack1, haystack2);
        // "hello" vs "hello" should be flagged as a conflict.
        assert_eq!(conflicts, vec![(0, 0)]);
    }

    // Test: one string being a prefix of the other is considered a conflict.
    #[test]
    fn test_prefix_conflict() {
        let haystack1 = vec!["he".to_string()];
        let haystack2 = vec!["hello".to_string()];
        let conflicts = compare_string_vectors(haystack1, haystack2);
        assert!(conflicts.is_empty());
    }

    // Test: strings that do not match should not produce any conflict.
    #[test]
    fn test_no_conflict() {
        let haystack1 = vec!["hello".to_string()];
        let haystack2 = vec!["world".to_string()];
        let conflicts = compare_string_vectors(haystack1, haystack2);
        assert!(conflicts.is_empty());
    }

    // Test: how empty strings are handled.
    #[test]
    fn test_empty_strings() {
        let haystack1 = vec!["".to_string(), "nonempty".to_string()];
        let haystack2 = vec!["anything".to_string(), "".to_string()];
        let conflicts = compare_string_vectors(haystack1, haystack2);
        // For the empty string cases:
        // - "" (haystack1[0]) vs "anything" (haystack2[0]): len=0 so the so conflict is not detected.
        // - "nonempty" (haystack1[1]) vs "" (haystack2[1]): len=0, so conflict is not detected.
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts.contains(&(0, 1)));
    }

    // Test: multiple strings with some conflicts.
    #[test]
    fn test_multiple_conflicts() {
        let haystack1 = vec!["foobar".to_string(), "barbecue".to_string(), "baz".to_string()];
        let haystack2 = vec!["foobar".to_string(), "barbecue".to_string(), "qux".to_string()];
        let conflicts = compare_string_vectors(haystack1, haystack2);
        // Analysis:
        // - "foo" (haystack1[0]) vs "foobar" (haystack2[0]): min length = 3, "foo" == "foo", conflict.
        // - "bar" (haystack1[1]) vs "barbecue" (haystack2[1]): min length = 3, "bar" == "bar", conflict.
        // Other comparisons do not result in conflicts.
        let mut conflicts_sorted = conflicts.clone();
        conflicts_sorted.sort();
        let mut expected = vec![(0, 0), (1, 1)];
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
        let conflicts = compare_string_vectors(haystack1, haystack2);
        assert!(
            conflicts.is_empty(),
            "Expected no conflict because the strings differ after the common prefix"
        );
    }
}
