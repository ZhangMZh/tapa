# Performance Tuning

**Purpose:** Identify and fix throughput bottlenecks in your TAPA design.

**When to use this:** When your design builds and runs correctly but measured throughput is below your target — for example, the kernel time is higher than expected or resource utilization is unexpectedly high.

## What you need

- A compiled `.xo` from `tapa --work-dir work.out compile ...`
- Reports in `work.out/` (synthesis reports, utilization data)
- Understanding of your design's expected throughput

## Prioritized checklist

Work through these checks in order — each is faster to fix than the next.

### 1. Check initiation interval (II) in synthesis reports

After `tapa compile`, check the HLS reports in `work.out/` for II violations:

- An II > 1 on a pipelined loop means it cannot accept a new iteration every
  cycle, which can reduce throughput.
- Look for scheduling warnings and the achieved II in the HLS report.

If the loop is not already pipelined, add `[[tapa::pipeline(1)]]` to request
II=1. If it still reports II>1, read the violation and restructure the
specified dependency, memory access, or data path. The attribute is a request,
not a guarantee that HLS can achieve II=1.

### 2. Check memory throughput — consider `async_mmap`

`mmap` leaves request scheduling to HLS and does not expose separate
request/response channels. If memory latency limits your task:

- Use `tapa::async_mmap` to overlap computation and memory access.
- Check the synthesis report for memory interface utilization.

### 3. Check stream depths — FIFOs too shallow?

A shallow FIFO can throttle a bursty producer even when the consumer's
average rate is sufficient. First rule out an element-count or protocol bug;
increasing depth cannot fix a structural deadlock. Then:

- Check waveforms from fast cosim (`-xsim_save_waveform`) to observe
  backpressure.
- If additional burst storage is needed, increase the depth in
  `tapa::stream<T, DEPTH>` and review the resource cost.

### 4. Find resource hotspots with `--enable-synth-util`

Run synthesis with utilization reporting enabled:

```bash
tapa --work-dir work.out synth \
  --enable-synth-util \
  --part-num xcu55c-fsvh2892-2L-e \
  --clock-period 3.33
```

TAPA runs an additional RTL synthesis pass and writes per-task resource counts to:

- `work.out/report.json` — machine-readable JSON
- `work.out/report.yaml` — human-readable YAML

Both files contain per-task LUT, FF, BRAM, URAM, and DSP counts. Use them to identify which tasks are consuming the most resources before proceeding to full implementation.

## Validation

After running `tapa synth --enable-synth-util`, confirm the reports were written:

```bash
ls work.out/report.json work.out/report.yaml
```

- `work.out/report.json` — machine-readable per-task resource counts (LUT, FF, BRAM, URAM, DSP)
- `work.out/report.yaml` — human-readable version of the same data

If these files are missing, synthesis either did not run or exited before the reporting step. Check the HLS log in `work.out/` for errors.

### Where per-task resource estimates come from

TAPA needs a resource estimate for each task to balance the design across SLRs during floorplanning and to report utilization. There are two sources, in order of increasing accuracy:

- **HLS estimates (available for each HLS-synthesized task).** After Vitis HLS synthesizes a task, TAPA reads its synthesis report for LUT, FF, BRAM, DSP, and URAM counts. These come "for free" with every `tapa synth` run, but **HLS estimates are coarse and frequently inaccurate** — they tend to undercount control and interconnect logic. Treat them as a rough guide, not a placement-quality number.

- **Post-RTL-synthesis estimates (with `--enable-synth-util`).** This runs an additional out-of-context Vivado synthesis pass per task on the generated RTL, producing materially more accurate resource counts. **Prefer this whenever the estimates matter** — i.e. before floorplanning a tight design or when interpreting the utilization report. It costs one extra Vivado synth per task.

## Improving Fmax with `tapa floorplan`

The checks above improve throughput. To improve clock frequency on a
supported multi-SLR device, run `tapa floorplan` between `synth` and `pack`.
It places tasks, balances resources, and pipelines SLR crossings.

Use floorplanning for cross-SLR timing failures or routing congestion, not for
long combinational paths inside a task. Those require HLS changes.

U-series implementation feedback uses `v++ --link`; the experimental V80
service-shell backend uses SLASH. See
[Floorplan for Timing Closure](floorplan.md) for the complete workflow,
including connectivity, `--run-impl`, DSE, result files, and troubleshooting.

## Advanced flags summary

| Flag | Description |
|------|-------------|
| `--enable-synth-util` | Run post-HLS RTL synthesis to collect per-task resource utilization. |

## If something goes wrong

```admonish warning
See [Common Errors](../troubleshoot/common-errors.md) for help with synthesis failures, II violation messages, and resource overflows.
```

---

**Next step:** [Learning Path](../tutorials/learning-path.md)
