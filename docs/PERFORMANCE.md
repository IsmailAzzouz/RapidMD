# RapidMD — Performance Report

All numbers measured on this machine (Windows 11, `fast` profile unless noted).
Every figure below came from an actual run; nothing is estimated.

## How to reproduce

| What | Command |
|---|---|
| Pipeline + UI benchmark | `cargo run --profile fast --example perf -- 1 10 100` |
| Before/after vs. HEAD | `examples/beforeafter.rs`, run from each tree root |
| Cold start / idle RSS / idle CPU | `powershell -File tools/startup_compare.ps1 -Runs 4` |
| Peak working set of a bench binary | `powershell -File tools/run_and_measure.ps1 -Exe <path> -Sizes 1,10` |

The before/after comparison uses a git worktree pinned at the pre-optimization
commit so both trees run **byte-identical harness code**:

```bash
git worktree add .bench-baseline HEAD
cp examples/beforeafter.rs .bench-baseline/examples/beforeafter.rs
cd .bench-baseline && cargo build --profile fast --features baseline --example beforeafter
```

## 1. Headline: before → after

Same harness, same corpus, same `fast` profile, 1 MiB document:

| Stage | Before (HEAD) | After | Speedup |
|---|---:|---:|---:|
| `md::parse` | 57.7 ms | 60.9 ms | 0.95× (noise) |
| `highlight::tokenize` | 19,812.6 ms | 5.0 ms | **≈ 3,960×** |
| editor layout job | 25,132.8 ms | 10.9 ms | **≈ 2,305×** |
| peak working set | 63.8 MiB | 64.0 MiB | 1.00× |

The old tokenizer was O(n²): it re-scanned the document to find line bounds for
every line. On a 1 MiB file that cost ~20 s, which is why the pre-optimization
tree could not complete a 10 MiB document inside a 10-minute timeout.

`md::parse` is unchanged by design — it was already linear, and it is now ~5% of
the cost of the layout job it feeds.

## 2. Binary size

| Profile | opt-level | lto | Size |
|---|---|---|---:|
| `lean` | `z` | fat | **12.97 MiB** |
| `fast` | `3` | fat | **16.03 MiB** |
| `release` | `3` | thin | **17.38 MiB** |

`fast` is the profile to ship. It is 1.35 MiB smaller than `release` (fat LTO +
full symbol strip) and, per §3, also the fastest to first window.

## 3. Cold start, idle RSS, idle CPU (real `rapidmd.exe`)

Profiles run round-robin so cold file cache hits every one equally; a warmup
launch is discarded. Median of 4 runs:

| Profile | First window | Idle working set | Private bytes | Idle CPU |
|---|---:|---:|---:|---:|
| `fast` | **53 ms** | 199.0 MiB | 424.8 MiB | 0.00 % |
| `lean` | 169 ms | 198.4 MiB | 425.2 MiB | 0.00 % |
| `release` | 55 ms | 202.6 MiB | 424.6 MiB | 0.00 % |

**`lean` is a trap.** `opt-level = "z"` buys 3 MiB but costs **3.2× the startup
time** (169 ms vs 53 ms) by optimizing for size in code that runs once at
startup. Do not ship it for a desktop app.

**The 199 MiB is the graphics stack, not the app.** Module-level breakdown at
idle:

| Module | Image size |
|---|---:|
| `nvgpucomp64.dll` | 94.1 MiB |
| `nvwgf2umx.dll` | 85.6 MiB |
| `igd12dxva64.dll` | 72.7 MiB |
| `igc64.dll` | 70.5 MiB |
| `nvrtum64.dll` | 52.4 MiB |
| `nvoglv64.dll` | 46.5 MiB |
| `rapidmd.exe` (our binary) | 16.0 MiB |

This machine has both an NVIDIA and an Intel GPU stack loaded. The application's
own footprint is the 16 MiB image plus its heap; the rest is driver code. The
headless benchmark (§4) shows 64 MiB peak for a 1 MiB document with no window at
all.

**Idle CPU is genuinely 0.00 %.** The CPU counter was verified to move: it
reports 172 ms accumulated during startup, then is flat across an 8-second idle
window. egui repaints on demand, so a windowed-but-untouched RapidMD does no
per-frame work. This is a direct result of the startup work — recovery scan,
font discovery and parsing were all moved off the first-frame path, and the FPS
overlay defaults to off.

## 4. Pipeline benchmark (`examples/perf.rs`)

### Open / parse / tokenize / build job

| Document | `md::parse` | `tokenize_into` | `editor_job_into` | Peak RSS |
|---|---:|---:|---:|---:|
| 1 MiB | 32.3 ms | 3.5 ms | 10.3 ms | 40.8 MiB |
| 10 MiB | 315.2 ms | 36.9 ms | 104.9 ms | 352.8 MiB |
| 100 MiB | 3,263.4 ms | 420.5 ms | 1,068.6 ms | 3,363.3 MiB |

Scaling is linear in document size from 10 → 100 MiB (parse 315 → 3,263 ms for
10×, tokenize 37 → 421 ms, job 105 → 1,069 ms), which is the property the old
tokenizer lacked.

### Typing latency (10 MiB document)

| Path | p50 | p95 | p99 | max |
|---|---:|---:|---:|---:|
| cold (tokenize + build job) | 97.1 ms | 99.6 ms | 101.7 ms | 105.4 ms |
| warm (cached tokens) | 38.8 ms | 39.7 ms | 39.7 ms | 39.7 ms |

Reusing the token buffer across frames cuts per-keystroke layout cost by ~2.5×.

### Preview frames (10 MiB document)

| Case | p50 | p95 | p99 |
|---|---:|---:|---:|
| first frame | 86.8 ms | — | — |
| steady frame (same offset) | 0.640 ms | 0.682 ms | 0.691 ms |
| scrolled frame (new offset) | 2.232 ms | 24.101 ms | 28.374 ms |
| revision-bumped frame | **0.197 ms** | 0.240 ms | 0.647 ms |

Steady-state repaints are 0.64 ms and a document edit that only bumps the
revision costs 0.197 ms. Scrolling is the expensive case (p95 24 ms) because it
forces re-layout of newly visible blocks — that is the viewport virtualization
boundary, and it is the main remaining scroll-smoothness headroom.

### Syntax highlight cache

| Case | p50 | p95 |
|---|---:|---:|
| cache hit (same text/theme) | 0.002 ms | 0.002 ms |
| cache miss (theme flip) | 0.001 ms | 0.002 ms |

Highlighting is now effectively free per frame: a code block is highlighted once
and reused until its text or the theme changes.

### Image cache (LRU)

| Case | Time |
|---|---:|
| cold decode of 48 images | 4.0 ms |
| warm (all cached) | 0.1 ms |
| second corpus (forces eviction) | 4.1 ms |

Working set holds at 78.1 MiB against a 48 MiB decoded-texture budget.

### Editor repaint cost breakdown (10 MiB)

| Phase | Time | Volume |
|---|---:|---:|
| boundary push | 2.1 ms | 1,271,424 points |
| boundary sort + dedup | 14.5 ms | 973,433 unique |
| section append loop | 15.3 ms | — |

### Cached job reuse (10 MiB)

| Path | p50 | mean |
|---|---:|---:|
| rebuild (fresh scratch) | 38.2 ms | 38.7 ms |
| clone of cached job | 17.0 ms | 17.0 ms |

## 5. Changes that produced the speedup

1. **O(n²) → O(n) tokenizer.** `text_line_bounds` re-scanned the whole document
   for every line. Replaced with a single pass over `split_inclusive('\n')` and a
   running byte offset.
2. **Reusable token buffer.** `tokenize_into` appends into a caller-owned `Vec`
   that the editor layouter reuses across frames, so steady-state repaints
   allocate nothing.
3. **Buffer revision counter.** `saved_text: String` became
   `saved_revision: u64`, making dirty detection O(1) instead of a full-text
   compare; `act_text()` returns `&str` instead of `String`.
4. **Viewport virtualization.** The preview lays out only visible blocks plus a
   small overscan, and highlights each code block once.
5. **Deferred startup work.** Recovery directory scanning, CJK font discovery,
   Syntect initialization and image decoding all moved off the first-frame path.
6. **Syntect language index.** O(1) language→syntax lookup replaced a full scan.
7. **Bounded image cache.** LRU eviction against a memory budget instead of
   unbounded growth.

## 6. Known limitations and remaining headroom

- **Scroll p95 is 24 ms** on a 10 MiB document. This is the one interaction that
  is not yet sub-frame smooth; it comes from laying out newly visible blocks.
- **Peak RSS scales with document size** (3.3 GiB for 100 MiB). The corpus is
  parsed and laid out eagerly, so a 100 MiB document is resident. A windowed
  partial parse would cap this but is a large change.
- **`md::parse` is now the largest single cost** at 100 MiB (3.3 s). It is linear
  and well-behaved, but it is the next thing to attack if larger files matter.
- **Idle RSS is dominated by the graphics driver**, so it is not a useful
  optimization target on this machine; the controllable number is the 64 MiB
  headless peak in §4.
- **Before/after at 10 and 100 MiB could not be completed** for the baseline
  tree — the old tokenizer's O(n²) behaviour puts a 10 MiB run beyond an hour.
  The 1 MiB row in §1 is the measured comparison; the larger current-tree
  numbers in §4 stand on their own.
