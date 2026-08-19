# P7 release evidence

This report defines the reproducible P7 acceptance commands and separates
repository evidence from platform work that must run on a real host. Generated
benchmark and size data belongs under `target/`; it is tied to one commit and
machine and is not presented as a hardware-independent promise.

## Reproducible commands

- `scripts/release-check.sh --quick` runs formatting, minimal/all-feature
  Clippy, the feature build matrix, minimal/default/all-feature tests, and the
  integrated headless example. Its output and
  `target/p7-release-report/metadata.txt` are the release-gate evidence for a
  given commit.
- `scripts/release-check.sh --full` additionally builds public documentation,
  packages the crate, and records stripped ThinLTO example sizes plus dependency
  trees for minimal, default, accessibility, webview, and all-feature builds.
- `scripts/run-p7-matrix.sh target/p7-benchmarks/current.csv` runs all seven
  retained-tree scenarios at 10, 100, 1,000, 5,000, 10,000, and 50,000 nodes.
- `scripts/run-p7-matrix.sh target/p7-benchmarks/current.csv
  target/p7-benchmarks/baseline.csv` rejects total-time p95 regressions above
  20 percent. `TGUI_BENCH_REGRESSION_PERCENT` changes that local threshold.

The current workspace completed the full matrix with
`TGUI_BENCH_SAMPLES=1`, emitting seven scenarios for each of 10, 100, 1,000,
5,000, 10,000, and 50,000 nodes. The stress companion emitted all six stress
scenarios. The generated CSVs are retained under `target/p7-benchmarks/`.

The matrix embeds commit, Rust version, OS/architecture, device label, theme,
font policy, DPI, window size, resource set, and sample count. Every row records
total/update/layout/paint/compile p50/p95/p99, RSS, dirty roots, rebuilds,
arena counters, Paint/Chunk/Batch/Pass/cache/upload/transient values, resource
counters, and all three budget snapshots. Submit timing, heap allocation bytes,
GPU time, and driver VRAM are emitted as `na` with an explicit unavailable
metadata row in the current headless `FrameMetrics`; they are not fabricated as
zero values.

## Platform evidence

The GitHub Actions host matrix compiles and tests minimal, default, and all
features on Windows, macOS, and Linux. A separate matrix checks each public
feature, and the MSRV job checks Rust 1.85. The `render`-feature test suite uses
a real headless wgpu adapter when one is available and covers resize, DPI,
texture upload, submission, delayed reclamation, and device recreation.

The repository now includes an optional `desktop`/`window` boundary with
`winit` and `WinitSurface`. It creates a real window, configures a transparent
surface, translates resize and scale-factor events, and exposes renderer device
recovery without leaking platform handles into the retained trees. The
`webview` feature remains an optional host capability; the headless example uses
the mock host so the minimal feature path stays dependency-free.

The local development host for this phase is macOS/aarch64. The following smoke
command completed on that host and exercised resize/DPI, transparency, surface
submission, and injected device-loss recovery:

```text
scripts/desktop-smoke.sh
```

Windows/Linux compilation and target-specific AccessKit adapter checks are
defined in `.github/workflows/ci.yml`; they require the corresponding CI/target
host and are not inferred from the local macOS run. Interactive window smoke on
those targets remains a release-host responsibility.

## API and packaging decision

This branch is an experimental API and is not compatible with the old main
branch public API. Native Host and accessibility adapters are optional; the
minimal core remains headless. Module ownership and cache/revision contracts
still cross `application`, `render`, `media`, and platform boundaries, so P7
keeps one crate. Splitting those modules now would expose internal snapshots and
increase compatibility surface before the platform adapter contract stabilizes.

## Observed status and known limits

- P6 Native Host, P6 Accessibility, and P7 contract suites pass under
  `cargo test --no-default-features`; the accessibility feature variant and the
  P7 all-feature contract suite also pass. `scripts/release-check.sh --quick`
  completed, including minimal/all-feature Clippy, the feature matrix,
  minimal/default/all-feature tests, and both headless example invocations.
- `cargo run --example p7_headless --no-default-features` is deterministic and
  prints state/event/text/image/animation/VirtualList/accessibility/native-host
  and revision evidence; the all-feature invocation is included in the release
  gate.
- `scripts/release-check.sh --quick` and `scripts/release-check.sh --full`
  completed. The full report includes `artifact-sizes.csv` and dependency trees
  for minimal, default, accessibility, webview, and all-feature builds. The
  current macOS/aarch64 sizes are 1,428,576; 1,445,152; 1,428,592; 1,445,168;
  and 1,445,136 bytes respectively.
- Benchmark outputs are commit- and machine-specific and should be regenerated
  for a different toolchain or host.

- The integrated example is GPU-free and deterministic; native-window behavior
  requires the platform smoke work described above.
- RSS comes from the host process. GPU time, driver VRAM, and global heap
  allocation counters are not sampled by the current headless `FrameMetrics`;
  they remain unavailable rather than being represented as zero.
- Benchmark thresholds compare the same scenario on comparable hardware. They
  are regression alarms, not FPS, RAM, or binary-size promises.
