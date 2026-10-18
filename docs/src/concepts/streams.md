# Streams

**Purpose:** Communicate between TAPA tasks using typed FIFO streams.

**Prerequisites:** [Tasks](tasks.md)

---

## Why this exists

Streams are the primary inter-task communication mechanism in TAPA. They are
typed, directional FIFOs that appear explicitly in task signatures, making data
flow visible in the source code. Unlike shared memory, streams enforce a
single-writer/single-reader discipline and make producer–consumer relationships
unambiguous to both the programmer and the compiler.

---

## Mental model

An inter-task stream instance lives in an upper-level task. Leaf tasks receive
directional references to it:

```cpp
// Upper-level task instantiates the stream and wires it to two leaf tasks
void Upper(/* ... */) {
  tapa::stream<float, 16> data_q("data_q");  // depth = 16 elements

  tapa::task()
      .invoke(Producer, data_q)   // Producer writes to data_q
      .invoke(Consumer, data_q);  // Consumer reads from data_q
}

// Leaf task signatures use directional references
void Producer(tapa::ostream<float>& out) { /* ... */ }
void Consumer(tapa::istream<float>& in)  { /* ... */ }
```

The `stream<T, Depth>` template parameter controls the hardware FIFO depth
(default: 2). A larger depth can reduce stalls at the cost of additional FPGA
resources.

---

## Blocking read and write

```cpp
void Task(tapa::istream<int>& in, tapa::ostream<int>& out) {
  int data = in.read();   // blocks until data is available
  out.write(data);        // blocks until space is available
}
```

The `<<` and `>>` operator aliases are equivalent:

```cpp
out << data;   // same as out.write(data)
in >> data;    // same as data = in.read()
```

---

## Non-blocking read and write

Non-blocking variants report whether they transferred a token. Always check
the result: a failed read consumes nothing, and a failed write sends nothing.

```cpp
int next_value = 42;
if (out.try_write(next_value)) {
  // next_value was written; compute the next token.
}

int data;
if (in.try_read(data)) {
  // data was consumed; keep it until every later write succeeds.
}
```

If a consumer must forward data through a full output stream, retain the value
and retry the write before reading another input token.

---

## Readiness checks

Check stream state before committing to a read or write:

```cpp
if (!in.empty())  { /* safe to read  */ }
if (!out.full())  { /* safe to write */ }
```

For non-destructive inspection, `peek` returns the front element and a validity
flag without consuming it:

```cpp
bool valid;
auto val = in.peek(valid);   // does not remove the token
if (valid && /* routing decision */) {
  in.read(nullptr);          // consume now
}
```

---

## End-of-Transaction (EoT)

A producer marks the end of a transaction by calling `close()`. The consumer
detects the marker with `try_eot()`, which inspects but does not consume it:

```cpp
// Producer
void Mmap2Stream(tapa::mmap<const float> mem, uint64_t n,
                 tapa::ostream<float>& stream) {
  for (uint64_t i = 0; i < n; ++i) {
    stream.write(mem[i]);
  }
  stream.close();  // send EoT token
}

// Consumer
void Stream2Mmap(tapa::istream<float>& stream, tapa::mmap<float> mem) {
  for (uint64_t i = 0;;) {
    bool eot;
    if (stream.try_eot(eot)) {
      if (eot) break;
      mem[i++] = stream.read(nullptr);
    }
  }
  stream.open();  // consume the EoT marker before the next transaction
}
```

### EoT loop helper macros

TAPA provides macros that encapsulate the non-blocking EoT check pattern:

- `TAPA_WHILE_NOT_EOT(stream)` — loops until `stream` delivers an EoT token;
  body executes only when a valid non-EoT token is available.
- `TAPA_WHILE_NEITHER_EOT(s1, s2)` — loops until either stream delivers EoT;
  body executes only when both have a valid token.
- `TAPA_WHILE_NONE_EOT(s1, s2, s3)` — three-stream variant.

```cpp
void Consumer(tapa::istream<int>& in, tapa::ostream<int>& out) {
  TAPA_WHILE_NOT_EOT(in) {
    out.write(in.read(nullptr));
  }
  in.open();
  out.close();
}
```

```admonish tip
After observing EoT, the consumer calls `stream.open()` to consume the marker.
`close()` and `open()` delimit transactions; they do not destroy and recreate
the stream. Consume the marker before starting the next transaction or ending
the task.
```

---

## Stream arrays

For parameterized designs, TAPA provides arrays of streams:

- `tapa::streams<T, N, Depth>` — array of N streams (instantiation in an
  upper-level task; `Depth` defaults to 2)
- `tapa::istreams<T, N>&` / `tapa::ostreams<T, N>&` — directional array
  references in leaf task signatures

When invoking `N` parallel instances of a leaf task, use `invoke<tag, N>(...)`
and TAPA distributes the array elements automatically:

```cpp
void InnerStage(int b, tapa::istreams<pkt_t, kN / 2>& in_q0,
                tapa::istreams<pkt_t, kN / 2>& in_q1,
                tapa::ostreams<pkt_t, kN>& out_q) {
  tapa::task().invoke<tapa::detach, kN / 2>(Switch2x2, b, in_q0, in_q1, out_q);
}
```

---

## Rules

- Always pass TAPA task stream ports by reference: `istream<T>&`,
  `ostream<T>&`. Passing one by value is rejected.
- A TAPA task port uses `istream<T>&` or `ostream<T>&`, not `stream<T>`.
  `stream` declares a channel; an upper-level task declares inter-task channels
  in its body. A top-level kernel likewise exposes external stream ports as
  `istream`/`ostream`.
- Each stream instance must have exactly one reader and exactly one writer.
- With `tapa::stream<T, Depth>`, software simulation enforces `Depth`: a full
  stream blocks the writer, matching hardware behavior. The compatibility
  alias `tapa::hls::stream<T, Depth>` is the exception: its simulation depth is
  effectively infinite while its hardware depth remains `Depth`.
- Stream depth is a hardware FIFO size. The FPGA resource used depends on depth:
  - **Depth = 1**: a one-element fall-through register.
  - **1 < Depth < 128**: synthesised from SRL shift-registers (no BRAM cost).
  - **Depth ≥ 128**: mapped to BRAM, except that **depth ≥ 4096 and element
    width ≥ 36 bits** takes precedence and maps to URAM.
- Default depth is 2, which costs only SRL resources.

Without floorplanning, a shallow FIFO becomes full at its configured depth,
even when its physical SRL storage has a minimum of four slots.

---

## Common mistakes

**Wrong** — stream passed by value (loses the required endpoint identity and
is rejected by TAPA):

```cpp
void Leaf(tapa::istream<float> in) { /* ... */ }  // missing &
```

**Right** — stream passed by reference:

```cpp
void Leaf(tapa::istream<float>& in) { /* ... */ }
```

**Wrong** — a channel where a port belongs (a `stream` has no direction, so
there is no interface for it to become):

```cpp
void Task(tapa::stream<float>& data_q) { /* ... */ }
```

```
error: tapa::stream parameter 'data_q' declares a channel rather than a port;
a task reads a channel through tapa::istream and writes one through
tapa::ostream
```

**Right** — the channel stays in the upper task, the port carries a direction:

```cpp
void Task(tapa::ostream<float>& data_q) { /* ... */ }

void Upper() {
  tapa::stream<float> data_q("data_q");
  tapa::task().invoke(Task, data_q);
}
```

---

## See also

- [Tasks](tasks.md)
- [Deadlocks & Hangs](../troubleshoot/deadlocks-and-hangs.md)
- [C++ API](../reference/api.md)

---

**Next step:** [Memory Access: mmap](mmap.md)
