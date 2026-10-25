# AI Engine Tasks

On a Versal device, a TAPA task can run on the AI Engine array instead of the
programmable logic (PL). The task keeps its C++ body; marking it moves it to an
AI Engine tile, and every stream between it and a PL task becomes a PLIO
connection. Software simulation is unchanged: an AI Engine task runs as an
ordinary task.

`tapa analyze` accepts AI Engine tasks today. `tapa synth` does not build them
yet and stops with a message naming the task.

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
