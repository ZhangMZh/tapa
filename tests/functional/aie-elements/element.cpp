#include "element.h"

void PairLoad(tapa::mmap<Pair> mem, int n, tapa::ostream<Pair>& out) {
  for (int i = 0; i < n; ++i) out.write(mem[i]);
}

void VecLoad(tapa::mmap<Vec3> mem, int n, tapa::ostream<Vec3>& out) {
  for (int i = 0; i < n; ++i) out.write(mem[i]);
}

[[tapa::target("aie")]]
void PairAdd(tapa::istream<Pair>& a, tapa::istream<Pair>& b,
             tapa::ostream<Pair>& c, int n) {
  for (int i = 0; i < n; ++i) {
    const Pair x = a.read();
    const Pair y = b.read();
    c.write({x.x + y.x, x.y + y.y});
  }
}

[[tapa::target("aie")]]
void VecAdd(tapa::istream<Vec3>& a, tapa::istream<Vec3>& b,
            tapa::ostream<Vec3>& c, int n) {
  for (int i = 0; i < n; ++i) c.write(a.read() + b.read());
}

void PairStore(tapa::istream<Pair>& in, tapa::mmap<Pair> mem, int n) {
  for (int i = 0; i < n; ++i) mem[i] = in.read();
}

void VecStore(tapa::istream<Vec3>& in, tapa::mmap<Vec3> mem, int n) {
  for (int i = 0; i < n; ++i) mem[i] = in.read();
}

void Elements(tapa::mmap<Pair> pair_a, tapa::mmap<Pair> pair_b,
              tapa::mmap<Pair> pair_c, tapa::mmap<Vec3> vec_a,
              tapa::mmap<Vec3> vec_b, tapa::mmap<Vec3> vec_c, int n) {
  tapa::stream<Pair> pair_a_q("pair_a_q"), pair_b_q("pair_b_q"),
      pair_c_q("pair_c_q");
  tapa::stream<Vec3> vec_a_q("vec_a_q"), vec_b_q("vec_b_q"), vec_c_q("vec_c_q");
  tapa::task()
      .invoke(PairLoad, pair_a, n, pair_a_q)
      .invoke(PairLoad, pair_b, n, pair_b_q)
      .invoke(PairAdd, pair_a_q, pair_b_q, pair_c_q, n)
      .invoke(PairStore, pair_c_q, pair_c, n)
      .invoke(VecLoad, vec_a, n, vec_a_q)
      .invoke(VecLoad, vec_b, n, vec_b_q)
      .invoke(VecAdd, vec_a_q, vec_b_q, vec_c_q, n)
      .invoke(VecStore, vec_c_q, vec_c, n);
}
