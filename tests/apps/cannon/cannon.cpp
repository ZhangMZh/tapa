// Copyright (c) 2024 RapidStream Design Automation, Inc. and contributors.
// All rights reserved. The contributor(s) of this file has/have agreed to the
// RapidStream Contributor License Agreement.

#include "cannon.h"

// Scatter n*n matrix into p*p blocks, each block.
void Scatter(tapa::mmap<const float> matrix_ptr,
             tapa::ostreams<float, p * p>& block) {
  const uint64_t kNumElems = (kN / p) * (kN / p);
  for (int j = 0; j < p * p; ++j) {
    for (uint64_t i = 0; i < kNumElems; ++i) {
      block[j].write(*matrix_ptr);
      ++matrix_ptr;
    }
  }
}

void Gather(tapa::mmap<float> matrix_ptr, tapa::istreams<float, p * p>& block) {
  const uint64_t kNumElems = (kN / p) * (kN / p);
  for (int j = 0; j < p * p; ++j) {
    for (uint64_t i = 0; i < kNumElems; ++i) {
      *matrix_ptr = block[j].read();
      ++matrix_ptr;
    }
  }
}

// Each PE holds one kB x kB block of A, B, and C, the rows of B and C as
// 8-lane vectors, in HLS and on the AI Engine alike (`--target
// ProcElem:aie`). An output vector accumulates in one register, one vector
// multiply-accumulate per k, and B moves around the ring as whole rows. The
// block ports arrive as frames on the AI Engine, leaving its two input and
// two output stream ports to the ring.
using vec8f = tapa::vec_t<float, 8>;
constexpr int kLanes = 8;
constexpr int kB = kN / p;
constexpr int kRowVecs = kB / kLanes;
constexpr int kVecs = kB * kRowVecs;
static_assert(kB % kLanes == 0, "a block row must be whole vectors");

// The rows of B a PE sends to its neighbor before it reads from the other,
// and the depth of the ring streams of B; A's streams hold as many floats.
constexpr int kRing = 2;
static_assert(kVecs % kRing == 0, "a block must be whole ring chunks");

void ProcElem([[tapa::aie_frame(kB * kB)]] tapa::istream<float>& a_fifo,
              [[tapa::aie_frame(kB * kB)]] tapa::istream<float>& b_fifo,
              [[tapa::aie_frame(kB * kB)]] tapa::ostream<float>& c_fifo,
              tapa::ostream<vec8f>& i_prev, tapa::istream<vec8f>& i_next,
              tapa::ostream<float>& j_prev, tapa::istream<float>& j_next) {
  float a[kB * kB];
  vec8f b[kVecs];
  vec8f c[kVecs];

init:
  for (int v = 0; v < kVecs; ++v) {
    [[tapa::unroll]] for (int lane = 0; lane < kLanes; ++lane) {
      a[v * kLanes + lane] = a_fifo.read();
      b[v][lane] = b_fifo.read();
    }
    c[v] = 0.f;
  }

outer:
  for (int l = 0; l < p; ++l) {
  compute:
    for (int i = 0; i < kB; ++i) {
      for (int jv = 0; jv < kRowVecs; ++jv) {
        vec8f acc = c[i * kRowVecs + jv];
        [[tapa::pipeline]] for (int k = 0; k < kB; ++k) {
          acc += tapa::make_vec<kLanes>(a[i * kB + k]) * b[k * kRowVecs + jv];
        }
        c[i * kRowVecs + jv] = acc;
      }
    }
  communicate:
    // Shift B to the previous row and A to the previous column, a chunk the
    // ring streams can hold at a time. Within a pipelined loop HLS may issue
    // a read ahead of an earlier write to another stream, and on the ring
    // every PE would then wait for a write no PE made, so the writes and the
    // reads go in separate loops. On the AI Engine, TAPA keeps the PE's
    // stream accesses in program order because the ring runs through it.
    for (int v0 = 0; v0 < kVecs; v0 += kRing) {
      const int x0 = v0 * kLanes;
      for (int v = v0; v < v0 + kRing; ++v) i_prev.write(b[v]);
      for (int x = x0; x < x0 + kRing * kLanes; ++x) j_prev.write(a[x]);
      for (int v = v0; v < v0 + kRing; ++v) b[v] = i_next.read();
      for (int x = x0; x < x0 + kRing * kLanes; ++x) a[x] = j_next.read();
    }
  }

finalize:
  for (int v = 0; v < kVecs; ++v) {
    [[tapa::unroll]] for (int lane = 0; lane < kLanes; ++lane) {
      c_fifo.write(c[v][lane]);
    }
  }
}

void Cannon(tapa::mmap<const float> a_vec, tapa::mmap<const float> b_vec,
            tapa::mmap<float> c_vec, uint64_t n) {
  assert(kN % p == 0);
  assert(n <= kN);

  tapa::streams<float, p * p> a("a");
  tapa::streams<float, p * p> b("b");
  tapa::streams<float, p * p> c("c");
  tapa::stream<float, kRing * kLanes> fifo_00_01("PE00->PE01");
  tapa::stream<float, kRing * kLanes> fifo_01_00("PE01->PE00");
  tapa::stream<float, kRing * kLanes> fifo_10_11("PE10->PE11");
  tapa::stream<float, kRing * kLanes> fifo_11_10("PE11->PE10");
  tapa::stream<vec8f, kRing> fifo_00_10("PE00->PE10");
  tapa::stream<vec8f, kRing> fifo_10_00("PE10->PE00");
  tapa::stream<vec8f, kRing> fifo_01_11("PE01->PE11");
  tapa::stream<vec8f, kRing> fifo_11_01("PE11->PE01");

  tapa::task()
      .invoke(Scatter, a_vec, a)
      .invoke(Scatter, b_vec, b)
      .invoke(ProcElem, a, b, c, fifo_00_10, fifo_10_00, fifo_00_01, fifo_01_00)
      .invoke(ProcElem, a, b, c, fifo_01_11, fifo_11_01, fifo_01_00, fifo_00_01)
      .invoke(ProcElem, a, b, c, fifo_10_00, fifo_00_10, fifo_10_11, fifo_11_10)
      .invoke(ProcElem, a, b, c, fifo_11_01, fifo_01_11, fifo_11_10, fifo_10_11)
      .invoke(Gather, c_vec, c);
}
