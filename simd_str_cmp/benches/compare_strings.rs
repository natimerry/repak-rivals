use std::sync::LazyLock;
use std::time::Duration;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkGroup, Criterion, Throughput};
use criterion::measurement::WallTime;
use simd_str_cmp::{compare_string_vectors, compare_string_vectors_naive, compare_string_vectors_simd, compare_string_vectors_simd_non_par};

// Generate test data for 16-byte strings.
static TEST_DATA_16: LazyLock<(Vec<String>, Vec<String>)> = LazyLock::new(|| {
    let size = 1000;
    let string_length = 16;
    let s = "a".repeat(string_length);
    let haystack1 = vec![s.clone(); size];
    let haystack2 = vec![s; size];
    (haystack1, haystack2)
});

// Generate test data for 32-byte strings.
static TEST_DATA_32: LazyLock<(Vec<String>, Vec<String>)> = LazyLock::new(|| {
    let size = 1000;
    let string_length = 32;
    let s = "a".repeat(string_length);
    let haystack1 = vec![s.clone(); size];
    let haystack2 = vec![s; size];
    (haystack1, haystack2)
});

// Generate test data for 64-byte strings.
static TEST_DATA_64: LazyLock<(Vec<String>, Vec<String>)> = LazyLock::new(|| {
    let size = 1000;
    let string_length = 64;
    let s = "a".repeat(string_length);
    let haystack1 = vec![s.clone(); size];
    let haystack2 = vec![s; size];
    (haystack1, haystack2)
});

static TEST_DATA_VERY_MASSIVE: LazyLock<(Vec<String>, Vec<String>)> = LazyLock::new(|| {
    let size = 1000;
    let string_length = 4096; // chosen at random
    let s = "a".repeat(string_length);
    let haystack1 = vec![s.clone(); size];
    let haystack2 = vec![s; size];
    (haystack1, haystack2)
});


static TEST_DATA_VERY_MASSIVE_SMALL_VEC: LazyLock<(Vec<String>, Vec<String>)> = LazyLock::new(|| {
    let size = 50;
    let string_length = 4096; // chosen at random
    let s = "a".repeat(string_length);
    let haystack1 = vec![s.clone(); size];
    let haystack2 = vec![s; size];
    (haystack1, haystack2)
});


pub fn criterion_benchmark_simd_vs_native_massive(c: &mut Criterion) {
    let mut group = c.benchmark_group("SIMD vs Native (Massive Strings)");
    group.measurement_time(Duration::from_secs(10));

    let (haystack1, haystack2) = TEST_DATA_VERY_MASSIVE.clone();
    group.throughput(Throughput::Elements(haystack1.len() as u64));

    run_bench(&mut group, "Portable SIMD 64-bit", &haystack1, &haystack2, compare_string_vectors);
    run_bench(&mut group, "SIMD AVX2 INTRINSICS", &haystack1, &haystack2, compare_string_vectors_simd);

    group.finish();
}

pub fn criterion_benchmark_simd_vs_native_64(c: &mut Criterion) {
    let mut group = c.benchmark_group("SIMD vs Native (64-byte Strings)");
    let (haystack1, haystack2) = TEST_DATA_64.clone();
    group.throughput(Throughput::Elements(haystack1.len() as u64));

    run_bench(&mut group, "Portable SIMD 64-bit", &haystack1, &haystack2, compare_string_vectors);
    run_bench(&mut group, "SIMD AVX2 INTRINSICS", &haystack1, &haystack2, compare_string_vectors_simd);
    run_bench(&mut group, "Native", &haystack1, &haystack2, compare_string_vectors_naive);

    group.finish();
}

pub fn criterion_benchmark_simd_vs_native_32(c: &mut Criterion) {
    let mut group = c.benchmark_group("SIMD vs Native (32-byte Strings)");

    let (haystack1, haystack2) = TEST_DATA_32.clone();
    group.throughput(Throughput::Elements(haystack1.len() as u64));

    run_bench(&mut group, "Portable SIMD 32-bit", &haystack1, &haystack2, compare_string_vectors);
    run_bench(&mut group, "SIMD AVX2 INTRINSICS", &haystack1, &haystack2, compare_string_vectors_simd);
    run_bench(&mut group, "Native", &haystack1, &haystack2, compare_string_vectors_naive);

    group.finish();
}

pub fn criterion_benchmark_simd_vs_native_small(c: &mut Criterion) {
    let mut group = c.benchmark_group("SIMD vs Native (Small Strings)");
    let (haystack1, haystack2) = TEST_DATA_16.clone();
    group.throughput(Throughput::Elements(haystack1.len() as u64));

    run_bench(&mut group, "Portable SIMD 16-bit", &haystack1, &haystack2, compare_string_vectors);
    run_bench(&mut group, "SIMD AVX2 INTRINSICS", &haystack1, &haystack2, compare_string_vectors_simd);
    run_bench(&mut group, "Native", &haystack1, &haystack2, compare_string_vectors_naive);

    group.finish();
}

pub fn criterion_benchmark_simd_vs_native_small_vector(c: &mut Criterion) {
    let mut group = c.benchmark_group("SIMD vs Native (Small Strings)");
    let (haystack1, haystack2) = TEST_DATA_VERY_MASSIVE_SMALL_VEC.clone();
    group.throughput(Throughput::Elements(haystack1.len() as u64));

    run_bench(&mut group, "AVX INTRINSICS (par)", &haystack1, &haystack2, compare_string_vectors_simd);
    run_bench(&mut group, "SIMD AVX2 INTRINSICS (non-par)", &haystack1, &haystack2, compare_string_vectors_simd_non_par);
    run_bench(&mut group, "Native", &haystack1, &haystack2, compare_string_vectors_naive);

    group.finish();
}


/// Helper function to register a benchmark for a given dataset and function.
fn run_bench(
    c: &mut BenchmarkGroup<WallTime>,
    label: &str,
    haystack1: &Vec<String>,
    haystack2: &Vec<String>,
    simd_fn: fn(&[String], &[String]) -> Vec<(usize, usize)>,
) {
    c.bench_function(label, |b| {
        b.iter(|| {
            let _ = simd_fn(black_box(haystack1), black_box(&haystack2));
        })
    });
}


criterion_group!(benches, criterion_benchmark_simd_vs_native_small_vector,criterion_benchmark_simd_vs_native_massive,criterion_benchmark_simd_vs_native_64,criterion_benchmark_simd_vs_native_32,criterion_benchmark_simd_vs_native_small);
criterion_main!(benches);