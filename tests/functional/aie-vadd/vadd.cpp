// Vector add with `Add` on the AI Engine: PL tasks move memory to and from
// the boundary streams, which become PLIOs. The streams are named after
// their variables, so `tapa aiesim` finds their software-simulation logs.

#include "vadd.h"

void Mmap2Stream(tapa::mmap<vec4f> mmap, int n, tapa::ostream<vec4f>& stream) {
  for (int i = 0; i < n; ++i) {
    stream << mmap[i];
  }
}

[[tapa::target("aie")]]
void Add(tapa::istream<vec4f>& a, tapa::istream<vec4f>& b,
         tapa::ostream<vec4f>& c, int n) {
  [[tapa::pipeline]] for (int i = 0; i < n; ++i) {
    c.write(a.read() + b.read());
  }
}

void Stream2Mmap(tapa::istream<vec4f>& stream, tapa::mmap<vec4f> mmap, int n) {
  for (int i = 0; i < n; ++i) {
    stream >> mmap[i];
  }
}

void VecAdd(tapa::mmap<vec4f> a, tapa::mmap<vec4f> b, tapa::mmap<vec4f> c,
            int n) {
  tapa::stream<vec4f> a_q("a_q");
  tapa::stream<vec4f> b_q("b_q");
  tapa::stream<vec4f> c_q("c_q");

  tapa::task()
      .invoke(Mmap2Stream, a, n, a_q)
      .invoke(Mmap2Stream, b, n, b_q)
      .invoke(Add, a_q, b_q, c_q, n)
      .invoke(Stream2Mmap, c_q, c, n);
}
