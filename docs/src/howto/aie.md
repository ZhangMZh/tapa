# AI Engine Tasks

On a Versal device, a TAPA task can run on the AI Engine array instead of the
programmable logic (PL). The task keeps its C++ body; marking it moves it to an
AI Engine tile, and every stream between it and a PL task becomes a PLIO
connection. Software simulation is unchanged: an AI Engine task runs as an
ordinary task.

`tapa analyze` accepts AI Engine tasks and `tapa aiesim` simulates them.
`tapa synth` does not build them yet and stops with a message naming the task.

The working example is `tests/functional/aie-vadd/`.

## Marking a task

Mark a leaf task, or an upper task to put its whole subtree on the AI Engine:

```cpp
using vec4f = tapa::vec_t<float, 4>;

[[tapa::target("aie")]]
void Add(tapa::istream<vec4f>& a, tapa::istream<vec4f>& b,
         tapa::ostream<vec4f>& c, int n) {
  [[tapa::pipeline]] for (int i = 0; i < n; ++i) c.write(a.read() + b.read());
}
```

The same choice can be made on the command line without editing the source,
so one source builds both a PL-only and a PL+AI Engine design. The mapping wins
over the attribute:

```bash
tapa analyze ... --target xilinx-vitis --target Add:aie
```

`TASK:TARGET` takes `aie`, `hls` or `ignore`; an unknown task name is an
error. Inside an AI Engine upper task, a descendant explicitly marked for
another target is an error. The top task cannot be an AI Engine task: the PL
kernel owns the control interface.

## What an AI Engine task may use

| Construct | On the AI Engine |
|---|---|
| `istream`, `ostream`, `istreams`, `ostreams` | Yes. Elements must be a multiple of 32 bits. |
| `read()`, `write()`, `>>`, `<<` | Yes, blocking. |
| `close`, `open`, `eot`, `try_eot`, `TAPA_WHILE_*EOT` | No. AI Engine streams carry no end-of-transmission token: loop over an element count. |
| `try_read`, `try_write`, `peek`, `empty`, `full` | No. AI Engine streams are blocking and expose no occupancy. |
| `mmap`, `async_mmap`, `mmaps`, `hmap` | No. Read memory in a PL task and stream the data in. |
| Scalars | Yes. |
| `[[tapa::pipeline]]`, `[[tapa::unroll]]` | Lowered to `chess_prepare_for_pipelining` and `chess_unroll_loop`. |

A PL task must not `close()` a stream that feeds an AI Engine task, because
the end-of-transmission token would be read as data.

Two habits decide AI Engine performance:

- Write the inner loop with `tapa::vec_t`. On the AI Engine a `vec_t<float, 4>`
  is one vector register, `a + b` is one vector add, and `acc += a * b` is one
  multiply-accumulate. Scalar loops stay scalar and run far slower.
- Use a 32-bit loop counter and `[[tapa::pipeline]]` on the hot loop. A 64-bit
  counter keeps the loop from being software-pipelined.

## Knobs

| Attribute | Where | Effect |
|---|---|---|
| `[[tapa::aie_frame(N)]]` | `istream`/`ostream` parameter of an AI Engine leaf | The stream arrives as frames of `N` elements in tile memory, filled by DMA, instead of through the core's stream port. The body is unchanged. |
| `[[tapa::aie_plio(W)]]` | `istream`/`ostream` parameter of an AI Engine task | PLIO width `W` of 32, 64 or 128 bits for a boundary stream. |
| `[[tapa::aie_location(col, row)]]` | AI Engine leaf task | Pins the task to a tile. Add a third argument to pick one instance of a task invoked several times. |

## Simulating the AI Engine tasks

Software simulation runs AI Engine tasks as ordinary tasks. To check the AI
Engine build of those tasks, record the streams a software simulation moves
and replay the ones that cross into the AI Engine:

```bash
mkdir logs
TAPA_STREAM_LOG_DIR=logs ./vadd-host 256
tapa --work-dir work.out analyze -f vadd.cpp -t VecAdd
tapa --work-dir work.out aiesim --stimulus logs --arg n=256 \
  --part-num xcvc1902-vsva2197-2MP-e-S
```

`tapa aiesim` generates the ADF graph under `work.out/aie/sim/`, compiles it
for `x86simulator`, feeds each PL-to-AI Engine stream from its
`<stream>.words` log, and compares every AI Engine-to-PL stream with its log.
A boundary stream is found by its variable name, so name the stream after its
variable: `tapa::stream<vec4f> a_q("a_q")`. Pass every top-task scalar the AI
Engine tasks read with `--arg`.

`--cycle-accurate` builds for hardware and runs `aiesimulator` instead, and
prints when the first and last beat of each output stream arrived and the mean
spacing between beats:

```text
c_q: 1024 of 1024 words, beats at 230.4..1862.4 ns (6.40 ns apart)
AI Engine simulation matches software simulation
```

On an AI Engine 1 device, a kernel reads about 32 bits from its input streams
per AI Engine cycle in total, however many stream ports it reads. That is one
128-bit beat every 3.2 ns at 1.25 GHz, so the two-input add above runs at
6.4 ns per output beat. Measure each kernel this way rather than planning on
the port count. A frame port (`[[tapa::aie_frame(N)]]`) is filled by DMA and
does not consume that budget.
