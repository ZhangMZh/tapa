// Copyright (c) 2024 RapidStream Design Automation, Inc. and contributors.
// All rights reserved. The contributor(s) of this file has/have agreed to the
// RapidStream Contributor License Agreement.

#pragma once

// `tapa/base/vec.h` uses CHECK_GE/CHECK_LE, so the target's logging must come
// first; `widthof` comes from util.h. The HLS pragmas inside the shared bodies
// are switched on by `TAPA_TARGET_XILINX_HLS_`, which the synthesis flow
// defines on the command line.
#include "tapa/xilinx/hls/logging.h"
#include "tapa/xilinx/hls/util.h"

namespace tapa {
namespace internal {

template <int W, bool S>
inline ap_int_base<W, false> materialize(const ap_range_ref<W, S>& proxy) {
  return static_cast<ap_int_base<W, false>>(proxy);
}

template <int W, bool S>
inline bool materialize(const ap_bit_ref<W, S>& proxy) {
  return static_cast<bool>(proxy);
}

}  // namespace internal
}  // namespace tapa

#include "tapa/base/vec.h"
