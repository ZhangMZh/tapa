#pragma once

#include <cstdint>

#include <tapa.h>

struct Pair {
  uint64_t x = 0;
  uint64_t y = 0;
};

using Vec3 = tapa::vec_t<float, 3>;

void Elements(tapa::mmap<Pair> pair_a, tapa::mmap<Pair> pair_b,
              tapa::mmap<Pair> pair_c, tapa::mmap<Vec3> vec_a,
              tapa::mmap<Vec3> vec_b, tapa::mmap<Vec3> vec_c, int n);
