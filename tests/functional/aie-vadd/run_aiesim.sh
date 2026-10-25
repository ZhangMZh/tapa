#!/bin/bash
# Replays the boundary streams of a software-simulation run into the AI Engine
# graph and checks what it writes back.
#
# usage: run_aiesim.sh TAPA HOST KERNEL [tapa aiesim flags...]
set -euo pipefail

tapa="$1"
host="$2"
kernel="$3"
shift 3

work="$(mktemp -d "${TEST_TMPDIR:-/tmp}/aie-vadd.XXXXXX")"
mkdir -p "${work}/log"
TAPA_STREAM_LOG_DIR="${work}/log" "${host}" 256
"${tapa}" --work-dir "${work}/work" analyze -f "${kernel}" -t VecAdd
# `//bazel:v++_env.sh` puts the host's glibc headers first for xsim; the AI
# Engine compiler's bundled gcc 8.3 cannot build x86sim against them.
env -u CPLUS_INCLUDE_PATH -u C_INCLUDE_PATH \
  "${tapa}" --work-dir "${work}/work" aiesim \
  --stimulus "${work}/log" --arg n=256 "$@"
