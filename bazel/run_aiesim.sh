#!/bin/bash
# Replays the boundary streams of a software-simulation run into the AI Engine
# graph and checks what it writes back.
#
# usage: run_aiesim.sh TAPA HOST KERNEL TOP [analyze flags...] -- [aiesim flags...]
set -euo pipefail

tapa="$1"
host="$2"
kernel="$3"
top="$4"
shift 4
analyze=()
while (($#)) && [[ "$1" != -- ]]; do
  analyze+=("$1")
  shift
done
(($#)) && shift

work="$(mktemp -d "${TEST_TMPDIR:-/tmp}/aiesim.XXXXXX")"
mkdir -p "${work}/log"
TAPA_STREAM_LOG_DIR="${work}/log" "${host}"
"${tapa}" --work-dir "${work}/work" analyze -f "${kernel}" -t "${top}" \
  "${analyze[@]}"
# `//bazel:v++_env.sh` puts the host's glibc headers first for xsim; the AI
# Engine compiler's bundled gcc 8.3 cannot build x86sim against them.
env -u CPLUS_INCLUDE_PATH -u C_INCLUDE_PATH \
  "${tapa}" --work-dir "${work}/work" aiesim --stimulus "${work}/log" "$@"
