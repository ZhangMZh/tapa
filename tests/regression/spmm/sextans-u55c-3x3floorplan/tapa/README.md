<!--
Copyright (c) 2024 RapidStream Design Automation, Inc. and contributors.
All rights reserved. The contributor(s) of this file has/have agreed to the
RapidStream Contributor License Agreement.
-->

This design was collected from [Sextans](https://github.com/linghaosong/Sextans).

[run_tapa.sh](./run_tapa.sh) currently builds
`src/sextans-no-const-para.cpp`, which avoids aggressively passing constants
through handshake interfaces. To build the constants variant instead, change
the `-f` argument to `src/sextans.cpp`.
