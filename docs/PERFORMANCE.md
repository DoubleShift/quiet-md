# Editor performance benchmarks

The README reports results from this procedure. Use the same scripts to reproduce it on your own machine.

## Environment

| Item | Value |
|---|---|
| Date | 2026-11-03 |
| Source | `0e92b51`, built with `cargo build --release --locked` |
| Machine | Apple M5 Pro, 48 GiB, macOS 26.5 |
| Window | 1280 × 800 pt at 2× scale, Craft theme |
| Document | 1,001,307 characters in 549 chapters with paragraphs, lists, tables and code, created by the [sample generator](../tools/perf/gen_library.py) |

## Method

The [harness](../tools/harness/README.md) replays [readme-performance.txt](../tools/harness/examples/readme-performance.txt). It opens the document, types 100 individual `x` characters at the end of the first paragraph, presses Backspace 100 times, then scrolls down 100 times.

After each operation, `bench` synchronously calls `window.draw` and records the frame processing time as `frame`. P50 is the median and P95 the 95th percentile. These figures measure in-app processing on one machine. End-to-end keyboard-to-screen latency also includes presentation time. No other apps were benchmarked.

## Results

| Operation | Frame P50 | Frame P95 | Maximum |
|---|---:|---:|---:|
| Typing | 5.06 ms | 5.78 ms | 6.94 ms |
| Backspacing | 4.99 ms | 5.55 ms | 5.69 ms |
| Scrolling | 2.08 ms | 4.14 ms | 9.23 ms |

Raw output in milliseconds:

```text
bench "type x" x100: step [p50 0.03 p95 0.05 max 5.29] frame [p50 5.06 p95 5.78 max 6.94] editor render [p50 3.77 p95 4.31 max 4.99] ms
bench "key backspace" x100: step [p50 0.02 p95 0.05 max 3.91] frame [p50 4.99 p95 5.55 max 5.69] editor render [p50 3.64 p95 4.12 max 4.24] ms
bench "wheel 900 500 40" x100: step [p50 0.01 p95 0.02 max 0.02] frame [p50 2.08 p95 4.14 max 9.23] editor render [p50 0.02 p95 0.03 max 1.83] ms
```

## Reproduce the benchmark

The test uses a temporary library and leaves `~/Documents/Quiet.md` untouched.

```sh
cargo build --release --locked --features devtools
python3 tools/perf/gen_library.py /tmp/quiet-perf/library
printf '{"english":true,"theme":"craft"}\n' > /tmp/quiet-perf/library/preferences.json
python3 tools/harness/run.py tools/harness/examples/readme-performance.txt /tmp/quiet-perf/library /tmp/quiet-perf/run
grep -a bench /tmp/quiet-perf/run/log
```

## Dragging the scrollbar

[scrollbar-drag.txt](../tools/harness/examples/scrollbar-drag.txt) opens the 1,000,000-character compilation, presses the page thumb and drags it 4 pt a step: 90 steps to halfway, 90 to the end, then 180 back to the top, with `bench` drawing a frame after each step. Every step lands on rows that may not have been laid out yet, so this is the editor's most expensive scrolling. `mean` is the average frame and `drops` the frames over 16.7 ms (one 60 Hz frame); screenshots at halfway, the end and the top check that the thumb is under the pointer and the page where the thumb says.

Two documents: the compilation as generated, and with `--tall-start` (three images, a 60-row table and two 120-line code blocks first). The thumb's length estimate counts rows not yet laid out at the average height of those on screen, so a tall first screen overestimates the document many times over.

Measured 2026-11-25 on an Apple M6 Mac mini (16 GiB, macOS 27.2.1), 1280 × 800 pt window, Craft theme, `--features devtools` release builds. Frame mean / drops (of 90) per quarter of the drag:

| Build | Document | Down 1 | Down 2 | Up 1 | Up 2 | Thumb under the pointer |
|---|---|---:|---:|---:|---:|---|
| v0.1.14 | as generated | 12.96 / 8 | 13.72 / 12 | 11.50 / 3 | 14.96 / 28 | yes |
| v0.1.14 | tall start | 5.85 / 6 | 3.03 / 0 | 3.00 / 0 | 6.34 / 9 | no: the page reaches its end at 1/10 of the track, the thumb stops there |
| 6df2232 | as generated | 14.21 / 21 | 14.90 / 19 | 12.03 / 6 | 11.16 / 1 | yes |
| 6df2232 | tall start | 3.25 / 5 | 3.35 / 0 | 1.84 / 0 | 3.47 / 4 | no (as v0.1.14) |
| 67d6145 | as generated | 16.19 / 33 | 23.19 / 79 | 11.02 / 4 | 10.83 / 7 | yes |
| 67d6145 | tall start | 16.70 / 24 | 26.23 / 83 | 23.57 / 75 | 15.83 / 32 | yes |
| this change | as generated | 12.77 / 7 | 15.20 / 23 | 11.88 / 6 | 9.81 / 0 | yes |
| this change | tall start | 13.10 / 8 | 14.93 / 19 | 11.18 / 4 | 10.26 / 1 | yes |

- With a tall start, v0.1.14 and 6df2232 are fast only because the page stops moving: holding the length estimated at the press, it reaches its end a tenth of the way down, and the rest of the drag redraws the same rows.
- 67d6145 made the page follow the thumb to the end, and aimed the page again on every frame in which newly measured rows had changed the estimate. During a drag that is nearly every frame, so each one laid out a second set of rows nobody saw: about 10 ms more a frame going down, and most frames dropped.
- Now a move aims the page by the latest estimate, and it is aimed again only once the pointer rests (80 ms) or the thumb is let go, the thumb held in place until the page is there. Frames are back to v0.1.14's; the thumb stays under the pointer, the end of the track is the end of the document, and letting go mid-drag leaves the thumb where it was (same pixels before and after).
- The rest of the cost, about 12 ms a frame going down, is laying out a screen of rows not seen before (text shaping); it has not changed since v0.1.14. The editor's own `render` stays under 0.2 ms.

Reproduce (the same temporary libraries; `run.py` copies them per run):

```sh
cargo build --release --locked --features devtools
python3 tools/perf/gen_library.py /tmp/quiet-perf/library
python3 tools/perf/gen_library.py --tall-start /tmp/quiet-perf/tall
for lib in library tall; do
  printf '{"english":true,"theme":"craft"}\n' > /tmp/quiet-perf/$lib/preferences.json
  python3 tools/harness/run.py tools/harness/examples/scrollbar-drag.txt /tmp/quiet-perf/$lib /tmp/quiet-perf/drag-$lib
  grep -a bench /tmp/quiet-perf/drag-$lib/log
done
```
