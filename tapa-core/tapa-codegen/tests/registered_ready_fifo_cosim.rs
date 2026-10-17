//! Verilator cosim for FIFO backpressure and forwarding behavior.

mod common;

use common::verilator::run_cosim;

const TESTBENCH: &str = r#"
#include <verilated.h>
#include "Vfifo_almost_full.h"
#include <cstdint>
#include <cstdio>
#include <deque>

static Vfifo_almost_full* dut;

static void posedge() {
    dut->clk = 1; dut->eval();
    dut->clk = 0; dut->eval();
}

static uint32_t next_random(uint32_t* state) {
    uint32_t value = *state;
    value ^= value << 13;
    value ^= value >> 17;
    value ^= value << 5;
    *state = value;
    return value;
}

int main(int argc, char** argv) {
    Verilated::commandArgs(argc, argv);
    dut = new Vfifo_almost_full;
    dut->clk = 0;
    dut->if_write_ce = 1;
    dut->if_read_ce = 1;
    dut->if_write = 0;
    dut->if_read = 0;
    dut->if_din = 0;

    dut->reset = 1;
    for (int i = 0; i < 8; ++i) posedge();
    dut->reset = 0;
    for (int i = 0; i < 8; ++i) posedge();

    if (!dut->if_full_n || dut->if_empty_n) {
        std::printf("FAIL: invalid state after reset\n");
        return 1;
    }

    // Ready must not have a combinational path from producer or consumer
    // inputs. It can change only after a clock edge updates FIFO state.
    const bool ready_before_toggle = dut->if_full_n;
    dut->if_write = 1;
    dut->if_read = 1;
    dut->if_din = 0xdeadbeefu;
    dut->eval();
    if ((bool)dut->if_full_n != ready_before_toggle) {
        std::printf("FAIL: ready changed without a clock edge\n");
        return 2;
    }
    dut->if_write = 0;
    dut->if_read = 0;

    // Fill until registered backpressure arrives, then hold a long consumer
    // stall. DEPTH=13 is the generated storage for a logical depth of 8.
    std::deque<uint32_t> expected;
    uint32_t next_value = 0;
    for (int cycle = 0; cycle < 64 && dut->if_full_n; ++cycle) {
        dut->if_write = 1;
        dut->if_din = next_value;
        dut->eval();
        if (dut->if_full_n) {
            expected.push_back(next_value++);
        }
        posedge();
    }
    dut->if_write = 0;
    if (dut->if_full_n || expected.size() < 8 || expected.size() >= 13) {
        std::printf("FAIL: registered backpressure used %zu entries\n", expected.size());
        return 3;
    }

    const uint32_t stalled_front = dut->if_dout;
    for (int cycle = 0; cycle < 32; ++cycle) {
        posedge();
        if (!dut->if_empty_n || (uint32_t)dut->if_dout != stalled_front) {
            std::printf("FAIL: stalled output was not stable\n");
            return 4;
        }
    }

    while (!expected.empty()) {
        dut->if_read = 1;
        dut->eval();
        if (!dut->if_empty_n || (uint32_t)dut->if_dout != expected.front()) {
            std::printf("FAIL: fill/drain ordering mismatch\n");
            return 5;
        }
        expected.pop_front();
        posedge();
    }
    dut->if_read = 0;
    dut->eval();
    if (dut->if_empty_n) {
        std::printf("FAIL: FIFO remained nonempty after drain\n");
        return 6;
    }

    for (int cycle = 0; cycle < 16 && !dut->if_full_n; ++cycle) posedge();
    if (!dut->if_full_n) {
        std::printf("FAIL: ready did not recover after drain\n");
        return 7;
    }

    // Randomized traffic includes long backpressure bursts and simultaneous
    // transfers. The software queue is the protocol-level reference model.
    const uint32_t item_count = 500;
    uint32_t sent = 0;
    uint32_t received = 0;
    uint32_t random_state = 0x5a17c9e3u;
    for (int cycle = 0; cycle < 200000 && received < item_count; ++cycle) {
        const uint32_t random_value = next_random(&random_state);
        const bool write = sent < item_count && dut->if_full_n &&
                           ((random_value & 3u) != 0u);
        const bool read = dut->if_empty_n &&
                          (cycle % 47 >= 19) &&
                          (((random_value >> 8) & 3u) != 0u);

        dut->if_write = write;
        dut->if_din = sent;
        dut->if_read = read;
        dut->eval();

        if (read) {
            if (expected.empty() || (uint32_t)dut->if_dout != expected.front()) {
                std::printf("FAIL: randomized ordering mismatch at item %u\n", received);
                return 8;
            }
            expected.pop_front();
            ++received;
        }
        if (write) {
            expected.push_back(sent++);
        }
        posedge();
    }

    if (sent != item_count || received != item_count || !expected.empty()) {
        std::printf("FAIL: sent=%u received=%u queued=%zu\n",
                    sent, received, expected.size());
        return 9;
    }

    std::printf("PASS: registered ready preserved capacity and FIFO ordering\n");
    return 0;
}
"#;

#[test]
fn registered_ready_fifo_is_lossless_under_backpressure() {
    run_cosim(
        "fifo_almost_full",
        &[
            ("DATA_WIDTH", 32),
            ("ADDR_WIDTH", 4),
            ("DEPTH", 13),
            ("GRACE_PERIOD", 1),
        ],
        &["relay_station.v"],
        &[],
        TESTBENCH,
    );
}

#[test]
fn shallow_fifo_backpressure_honors_requested_depth() {
    let testbench = r#"
#include <verilated.h>
#include "Vfifo.h"
#include <cstdint>
#include <cstdio>
#include <cstdlib>

static void check(bool condition, const char* message) {
    if (!condition) {
        std::printf("FAIL: %s\n", message);
        std::exit(1);
    }
}

int main(int argc, char** argv) {
    Verilated::commandArgs(argc, argv);
    constexpr uint32_t kDepth = REQUESTED_DEPTH;
    Vfifo dut;
    dut.clk = 0;
    dut.reset = 1;
    dut.if_write_ce = 1;
    dut.if_read_ce = 1;
    dut.if_write = 0;
    dut.if_read = 0;
    dut.if_din = 0;
    const auto posedge = [&]() {
        dut.clk = 1; dut.eval();
        dut.clk = 0; dut.eval();
    };
    dut.eval();
    posedge();
    dut.reset = 0;
    dut.if_write = 1;
    for (uint32_t value = 0; value < kDepth; ++value) {
        dut.if_din = 0x100000000ull | value;
        dut.eval();
        check(dut.if_full_n, "ready must accept every configured entry");
        posedge();
    }
    check(!dut.if_full_n, "ready must fall at the requested logical depth");

    dut.if_din = 0xabcdef01ull;
    for (int cycle = 0; cycle < 3; ++cycle) {
        dut.eval();
        check(!dut.if_full_n && dut.if_empty_n && dut.if_dout == 0x100000000ull,
              "pending writes cannot change the full FIFO's front token");
        posedge();
    }
    dut.if_write = 0;
    dut.if_read = 1;
    for (uint32_t value = 0; value < kDepth; ++value) {
        dut.eval();
        check(dut.if_empty_n && dut.if_dout == (0x100000000ull | value),
              "drain must preserve every accepted token and its EOT bit");
        posedge();
    }
    check(!dut.if_empty_n && dut.if_full_n, "drain must leave the FIFO empty");
    std::printf("PASS: shallow FIFO honors requested depth %u\n", kDepth);
    return 0;
}
"#;
    for depth in [2, 3] {
        run_cosim(
            "fifo",
            &[("DATA_WIDTH", 33), ("ADDR_WIDTH", 2), ("DEPTH", depth)],
            &["fifo.v", "fifo_fwd.v", "fifo_srl.v", "fifo_bram.v"],
            &[],
            &testbench.replace("REQUESTED_DEPTH", &depth.to_string()),
        );
    }
}

#[test]
fn depth_one_fifo_preserves_a_buffered_token_until_read() {
    run_cosim(
        "fifo",
        &[("DATA_WIDTH", 33), ("ADDR_WIDTH", 1), ("DEPTH", 1)],
        &["fifo.v", "fifo_fwd.v", "fifo_srl.v", "fifo_bram.v"],
        &[],
        r#"
#include <verilated.h>
#include "Vfifo.h"
#include <cstdint>
#include <cstdio>
#include <cstdlib>

static void check(bool condition, const char* message) {
    if (!condition) {
        std::printf("FAIL: %s\n", message);
        std::exit(1);
    }
}

int main(int argc, char** argv) {
    Verilated::commandArgs(argc, argv);
    Vfifo dut;
    dut.clk = 0;
    dut.reset = 1;
    dut.if_write_ce = 1;
    dut.if_read_ce = 1;
    dut.if_write = 0;
    dut.if_read = 0;
    dut.if_din = 0;
    const auto posedge = [&]() {
        dut.clk = 1; dut.eval();
        dut.clk = 0; dut.eval();
    };
    dut.eval();
    posedge();
    dut.reset = 0;
    dut.eval();
    check(dut.if_full_n && !dut.if_empty_n, "reset leaves FIFO empty");

    // Include the end-of-transaction bit in the buffered stream token.
    const uint64_t first = 0x100000001ull;
    const uint64_t second = 0xabcdef01ull;
    dut.if_write = 1;
    dut.if_din = first;
    dut.eval();
    check(dut.if_full_n && dut.if_empty_n && dut.if_dout == first,
          "empty FIFO forwards the offered token before the clock edge");
    posedge();

    // A producer may keep its next write pending while ready is low. No
    // transfer occurs, so every bit of the buffered token must stay stable.
    dut.if_din = second;
    dut.eval();
    for (int cycle = 0; cycle < 3; ++cycle) {
        check(!dut.if_full_n && dut.if_empty_n && dut.if_dout == first,
              "backpressure preserves the buffered token");
        posedge();
    }
    check(!dut.if_full_n && dut.if_empty_n && dut.if_dout == first,
          "buffered token survives every stalled clock edge");

    dut.if_read = 1;
    dut.if_read_ce = 0;
    posedge();
    check(!dut.if_full_n && dut.if_empty_n && dut.if_dout == first,
          "disabled read cannot consume the buffered token");

    // A full FIFO still rejects the pending write on the edge that reads
    // its old token. Ready returns afterwards; do not add a replacement path.
    dut.if_read_ce = 1;
    dut.eval();
    check(!dut.if_full_n && dut.if_dout == first,
          "reading a full FIFO does not combinationally raise ready");
    posedge();
    check(dut.if_full_n && dut.if_empty_n && dut.if_dout == second,
          "pending write forwards after the buffered token is consumed");

    // Both sides accept the second token directly through the now-empty
    // FIFO. It must not remain buffered or be delivered a second time.
    posedge();
    dut.if_write = 0;
    dut.if_read = 0;
    dut.eval();
    check(dut.if_full_n && !dut.if_empty_n,
          "simultaneous empty-FIFO write and read leave no stored token");
    std::printf("PASS: depth-one FIFO preserves stalled data and forwarding\n");
    return 0;
}
"#,
    );
}
