# Vitis Cosimulation

**Purpose:** Run Vitis hardware emulation to validate the kernel with the
platform shell and XRT after fast cosim passes.

**When to use this:** When you need Vitis's platform-level simulation rather
than TAPA's simplified fast-cosim environment. Even a simple design can take
several minutes, and large designs take much longer. Run
[Fast Hardware Simulation](fast-cosim.md) first to catch RTL logic errors.

## What you need

- A `.xo` kernel object from `tapa compile`
- Vitis and XRT installed (Linux only)
- The target platform string (e.g., `xilinx_u55c_gen3x16_xdma_3_202210_1`)

## Commands

### Generate the hardware emulation bitstream

```bash
platform=xilinx_u55c_gen3x16_xdma_3_202210_1
xo=vecadd.xo  # output of tapa compile

v++ -o vadd.$platform.hw_emu.xclbin \
  --link \
  --target hw_emu \
  --kernel VecAdd \
  --platform "$platform" \
  "$xo"
```

Replace `$platform` with your actual target platform string, `VecAdd` with
your top-level kernel name, and `$xo` with your TAPA-generated XO. This step
usually takes minutes and can take much longer for a large design.

### Run the hardware emulation

```bash
./vadd --bitstream=vadd.$platform.hw_emu.xclbin 1000
```

The same host executable used for software simulation and fast cosim runs unchanged here — only the `--bitstream` argument changes.

## Expected output

XRT and hardware-emulation logs vary by Vitis version. The application's
result should look like:

```
...
kernel time: 1.00021 s
PASS!
...
INFO: [HW-EMU 06-1] All the simulator processes exited successfully
```

```admonish warning title="Do not benchmark with hardware emulation"
The host's `kernel time` and Vitis performance reports describe an emulated
run with approximate global-memory and interconnect models. They are not
on-board performance measurements. Use them for diagnosis, and measure real
performance on the target board.
```

## Validation

The run is correct when:

1. The host process exits successfully.
2. The application's correctness check prints `PASS!`.
3. Vitis reports that its simulator processes exited successfully.

```admonish tip
Use a small dataset for hardware emulation runs. Large datasets cause proportionally long simulation times because every clock cycle is simulated in software.
```

## If something goes wrong

```admonish warning
See [Cosimulation Issues](../troubleshoot/cosim-issues.md) for diagnosis steps. Common issues include missing XRT environment variables, platform string mismatches, and kernel name mismatches between the `--kernel` flag and the TAPA top-level function name.
```

---

**Next step:** [Build & Run on Board](build-and-run.md)
