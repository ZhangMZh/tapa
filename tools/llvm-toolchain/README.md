# LLVM toolchain

TAPA uses `toolchains_llvm` to configure its Bazel C/C++ toolchain. On Linux
x86_64, the compiler, linker, and binary utilities used by Bazel C/C++ actions
are static binaries from hermetic-llvm, with headers and compiler-rt libraries
from the matching official LLVM release. These action tools run without host
libraries such as `libxml2.so.2` and `libz.so.1`. Auxiliary LLVM programs
retained from the official archive keep their upstream runtime dependencies.
Other execution platforms use the distributions selected by `toolchains_llvm`
in `MODULE.bazel`.

The target C and C++ headers, startup files, and libraries still come from the
host distribution. Install its libc development files and libstdc++ headers
and libraries. The static compiler binaries do not provide a bundled sysroot
or make generated programs independent of those target libraries.

## Published archives

The Linux x86_64 toolchain combines two pinned archives:

| Source | Archive | SHA256 |
|---|---|---|
| [hermetic-llvm 23.1.2](https://github.com/hermeticbuild/hermetic-llvm/releases/tag/llvm-23.1.2-2) | `llvm-toolchain-minimal-23.1.2-linux-amd64-musl.tar.zst` | `2808ea0503af3656373fc141ddc7d082b53c7f858efd5245aed0241734257348` |
| [Official LLVM 23.1.2](https://github.com/llvm/llvm-project/releases/tag/llvmorg-23.1.2) | `LLVM-23.1.2-Linux-X64.tar.xz` | `b5ed9675149cc837c282e9b6962c276c9fa62863d5b2f91537b60848552995b7` |

The [repository rule](../../bazel/llvm_toolchain.bzl) uses the hermetic-llvm
archive's static multicall `bin/llvm` and compiler/tool symlinks on Linux
x86_64. The official archive supplies `include/` and `lib/`, including the
`lib/clang/23` compiler resources and ASan libraries required by
`--config=asan`. Every configured C/C++ action tool uses the static binary on
Linux x86_64. Auxiliary programs absent from the static archive, such as
`clang-apply-replacements`, remain available from the official archive and
retain its host runtime dependencies.
The minimal static archive contains no compiler-rt libraries, so it cannot
replace the official resource archive by itself.

The repository assembly preserves the layout expected by `toolchains_llvm`
and checks both archive hashes. Updating it requires matching compiler and
resource versions and reviewing the published archive contents.

## Optional source build

`build-llvm-toolchain.sh` and the `Build LLVM toolchain (hermetic)` workflow
provide an optional way to build custom archives. They are not required for
the published static toolchain above. The builder currently has pinned sources
for LLVM 18.1.8 and 14.0.0, and the workflow builds those versions in Ubuntu
and AlmaLinux containers.

The source build disables LLVM's optional libxml2, zlib, zstd, terminfo,
libedit, libpfm, curl, and httplib dependencies. It builds Clang and LLD for
`X86;AArch64`, includes compiler-rt, and excludes tests, documentation,
examples, benchmarks, and bindings. Its binaries still depend on the build
distribution's libc and C++ runtime.

The script rejects references to disabled libraries and checks C++17
`<filesystem>` and AddressSanitizer. To build one custom archive:

```sh
docker run --rm -v "$PWD":/src:ro -v /tmp/llvm-out:/out \
  ubuntu:22.04 bash /src/tools/llvm-toolchain/build-llvm-toolchain.sh \
  18.1.8 ubuntu-22.04 /tmp/llvm-build
```

The artifact and SHA256 file land in `/tmp/llvm-out`. The manual workflow
publishes the same archive layout as assets of the chosen release tag.
Configure custom archives through `toolchains_llvm`'s per-platform `urls`,
`sha256`, and `strip_prefix` overrides only after their URLs and hashes exist.

## Version policy

All Linux x86_64 distributions use the same LLVM 23.1.2 static C/C++ action
tools and matching official resources. Static musl binaries remove these
tools' dependency on a distribution-specific glibc or libstdc++ runtime. Target
library compatibility still depends on the distribution's development files.
Other execution platforms retain the `toolchains_llvm` distribution selection
for LLVM 23.1.2.

The optional source builder's 18.1.8 and 14.0.0 pins are independent of the
active Bazel toolchain. Its source hashes live in `build-llvm-toolchain.sh`
(`SRC_SHA256`); add a row when building another version.
