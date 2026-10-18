# Runtime Flags

This page covers environment variables and host executable flags that control
software simulation, fast cosimulation, and XRT execution.

---

## Environment Variables

Set these variables before launching the host process. Some are read once
on first use rather than at startup, so changing them from within the program
is not a supported way to reconfigure execution.

| Variable | Default | Description |
|----------|---------|-------------|
| `TAPA_CONCURRENCY` | Number of physical CPU cores | Number of worker threads running task coroutines. Set to `1` for a simpler scheduling interleaving. Has no effect on HLS compilation parallelism (`-j`). |
| `TAPA_STREAM_LOG_DIR` | (unset — logging disabled) | Existing directory for stream write logs. TAPA writes one file per named stream; unnamed streams are not logged. Useful for tracing data during software simulation. |
| `TAPA_STREAM_LEFTOVER_LOG_COUNT` | `10` | Maximum number of leftover tokens printed for a nonempty stream when it is destroyed. Nonpositive values are invalid. |
| `TAPA_STALL_WARN_SECONDS` | `10` | Warn when software simulation makes no successful stream progress for this many seconds, naming a blocked channel. Set `0` to disable. Suppressed while a kernel instance is running, since waiting on hardware or RTL is not a stall. |
| `FRT_STREAM_DEBUG` | (unset) | When set, log every successful stream read and write in the DPI layer. Produces high-volume output; use only for targeted debugging. |
| `FRT_COSIM_YIELD` | `1` (enabled) | When enabled, the DPI layer sleeps briefly on empty reads or full writes so other cosim processes can progress. Disable with `0` to busy-wait instead. |
| `FRT_COSIM_TIMEOUT_SECONDS` | `0` (disabled) | Maximum wall-clock time for a fast-cosim simulator process. On expiry, TAPA terminates the simulator process group and reports a timeout. Set a positive integer to enable it. |
| `FRT_XSIM_LEGACY` | `0` | Set to `1` to use the legacy xelab command-line format for older Vivado versions. |
| `FRT_XOCL_BDF` | (unset) | PCIe Bus:Device:Function for XRT/OpenCL device selection. Equivalent to the `-xocl_bdf` gflag. |
| `TAPA_HOME` | (unset) | TAPA installation prefix, e.g. `/opt/tapa`. Only needed as an override: cosim normally finds its DPI libraries through the library path `tapa g++` links into the host binary. `install.sh` sets it for you (`/etc/profile.d/tapa.sh` for a system-wide install, your shell profile otherwise). Set it by hand if you relocated the installation or copied a host binary to a machine without it. |

### Example: single-worker simulation

```bash
TAPA_CONCURRENCY=1 ./vadd
```

### Example: enable stream logging

```bash
mkdir -p /tmp/tapa-stream-logs
TAPA_STREAM_LOG_DIR=/tmp/tapa-stream-logs ./vadd
```

See [Software Simulation](../howto/software-simulation.md) for more on stream logging and debugging.

---

## Host Executable Flags (Fast Cosim)

When the host executable is invoked with `--bitstream=vadd.xo`, it runs fast hardware cosimulation instead of software simulation. The following flags control cosim behavior. They are passed directly on the host executable command line.

```admonish note
These are gflags parsed by the host executable, not options to the `tapa`
compiler. Gflags accepts both `-cosim_work_dir` and `--cosim_work_dir`.
```

| Flag | Description |
|------|-------------|
| `-xsim_part_num <part>` | Override the FPGA part recorded in the archive. Usually unnecessary. |
| `-cosim_work_dir <dir>` | Persistent working directory for simulation artifacts. Without this flag, a temporary directory is used and deleted after the run. |
| `-xsim_save_waveform` | Save simulation waveforms to a `.wdb` file in the work directory. Pair with `-cosim_work_dir`; without it, the temporary directory and all waveforms are deleted after the run. |
| `-xsim_start_gui` | Open the Vivado GUI for interactive debugging during simulation. |
| `-cosim_simulator <backend>` | Simulator backend: `xsim` (default, Linux only, requires Vivado) or `verilator` (Linux/macOS, no Vivado required, needs Verilator 5.044+). |
| `-cosim_timeout_seconds <seconds>` | Stop the simulator process group after this much wall-clock time and report a timeout. `0` (the default) disables the timeout. |
| `-cosim_setup_only` | Run simulation setup only, then stop before executing the simulation. Useful for inspecting generated simulation files before committing to a full run. |
| `-cosim_resume_from_post_sim` | Skip re-running the simulation and jump directly to post-simulation checks. Use after a completed simulation to re-run checks without re-simulating. |
| `-cosim_work_dir_parallel` | Create a unique subdirectory per instance when running multiple concurrent simulations, preventing work directory collisions. |
| `-xocl_bdf <BDF>` | Select the XRT/OpenCL device by PCIe Bus:Device:Function. Equivalent to `FRT_XOCL_BDF`; used for hardware emulation or on-board execution, not fast cosim. |

### Example: save waveforms from a named work directory

```bash
./vadd --bitstream vadd.xo \
  -cosim_work_dir ./cosim_work \
  -xsim_save_waveform \
  1000
```

### Example: setup, manual xsim run, and resume

```bash
cosim="$PWD/cosim_work"

# Generate the simulation environment.
./vadd --bitstream=vadd.xo \
  -cosim_work_dir "$cosim" -cosim_setup_only 1000

# Run xsim yourself; resume does not perform this step.
TAPA_DPI_CONFIG="$(cat "$cosim/dpi_config.json")" \
  HOME="$cosim/run" \
  vivado -mode batch -source "$cosim/run_cosim.tcl"

# Reopen the simulated buffers and run the host-side checks.
./vadd --bitstream=vadd.xo \
  -cosim_work_dir "$cosim" -cosim_resume_from_post_sim 1000
```

See [Fast Hardware Simulation](../howto/fast-cosim.md) for the complete
workflow and other cosim options.
