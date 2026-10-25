#!/bin/bash
# Hybrid cosimulation: the PL half as RTL, the AI Engine task in software,
# driven by the unchanged host binary.
#
# usage: run_hybrid.sh TAPA HOST KERNEL PLATFORM
set -euo pipefail

tapa="$1"
host="$2"
kernel="$3"
platform="$4"

work="$(mktemp -d "${TEST_TMPDIR:-/tmp}/aie-vadd.XXXXXX")"
"${tapa}" --work-dir "${work}/work" compile -f "${kernel}" -t VecAdd \
  --platform "${platform}" --skip-aie -o "${work}/vadd.xo"
"${host}" --bitstream="${work}/vadd.xo" 64
