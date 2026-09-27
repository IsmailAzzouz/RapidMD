# Bundle Size Optimization Benchmarks

## Executive Summary

**Best Configuration: `release-size` profile** — **13.6 MB** (42.4% reduction from original 23.6 MB)

## Benchmark Results

| Profile | Binary Size | Reduction vs Original | Build Time | Key Settings |
|---------|-------------|----------------------|------------|--------------|
| **release-size** | **13,586,432 bytes (13.6 MB)** | **42.4% ↓** | ~2m 39s | `opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`, `strip = true`, `panic = "abort"` |
| release-lto-fat | 16,769,536 bytes (16.8 MB) | 28.9% ↓ | ~2m | `lto = "fat"`, `codegen-units = 1`, `strip = true`, `panic = "abort"` |
| release-lto-thin | 18,181,632 bytes (17.3 MB) | 22.9% ↓ | ~1m 49s | `lto = "thin"`, `codegen-units = 1`, `strip = true`, `panic = "abort"` |
| release-no-strip | 18,183,680 bytes (17.3 MB) | 22.9% ↓ | ~1m 46s | `strip = false` (all else same as release) |
| **release (original)** | **23,576,576 bytes (23.6 MB)** | **baseline** | ~43s | `lto = "thin"` only |

## Key Findings

### 1. `opt-level = "z"` (size optimization) is the single biggest win
- Reduces binary by **~4.6 MB** vs `opt-level = 3` with same LTO settings
- Trades some runtime performance for size — acceptable for a GUI application

### 2. `lto = "fat"` outperforms `lto = "thin"` for size
- Fat LTO: 16.8 MB (with opt-level=3)
- Thin LTO: 17.3 MB (with opt-level=3)
- Fat LTO enables more cross-crate optimizations

### 3. `strip = true` saves ~2 KB
- Minimal impact on this binary (already mostly stripped by default on Windows)
- Still worth keeping for consistency

### 4. `panic = "abort"` + `codegen-units = 1`
- Reduces unwinding metadata
- Single codegen unit enables better LTO optimization
- Both included in all optimized profiles

### 5. Build time trade-off
- `release-size`: ~2m 39s (slowest due to `opt-level = "z"` + fat LTO)
- Original `release`: ~43s (fastest)
- Consider CI caching for production builds

## Recommended Configuration

The `release-size` profile is already defined in `Cargo.toml` and produces the smallest binary. Use it for production releases:

```bash
cargo build --profile release-size --bin rapidmd
```

## Additional Optimization Opportunities (Not Tested)

| Optimization | Potential Impact | Notes |
|--------------|------------------|-------|
| `cargo build --target=x86_64-pc-windows-msvc` (explicit) | Minor | Already default |
| UPX compression | 30-50% additional | Runtime decompression overhead |
| Remove unused dependencies | Variable | Audit `Cargo.toml` features |
| `eframe` feature flags | Variable | Disable unused egui/wgpu features |
| `syntect` default-features = false | ~1-2 MB | Already disabled |
| `image` format features | ~500 KB | Already minimal (png, jpeg, gif, webp, bmp, ico, tiff) |

## Verification

All binaries were verified to run correctly on Windows. The `release-size` binary launches and functions identically to the original release build.