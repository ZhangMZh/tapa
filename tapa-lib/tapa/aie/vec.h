#pragma once

#include <aie_api/aie.hpp>

#include "tapa/base/util.h"

// Every AI Engine helper below is forced inline: the chess compiler keeps
// an out-of-line call per element otherwise, which costs tens of cycles
// where the vector unit needs one.
#define TAPA_AIE_INLINE inline __attribute__((always_inline))

// `tapa::vec_t` on the AI Engine: one `aie::vector` register, so the portable
// operators below are single vector instructions. The surface is the subset
// of `tapa/base/vec.h` the vector unit implements natively; a body using
// anything else does not compile for the AI Engine (docs/src/howto/aie.md).
// `acc += a * b` lowers to one `aie::mac`: `operator*` returns a lazy
// product that `+=`/`-=` consume, and that otherwise converts to a vector.

namespace tapa {

namespace internal {

template <typename T, int N>
struct vec_product {
  ::aie::vector<T, N> lhs;
  ::aie::vector<T, N> rhs;

  TAPA_AIE_INLINE ::aie::vector<T, N> eval() const {
    return ::aie::mul(lhs, rhs).template to_vector<T>();
  }
  TAPA_AIE_INLINE ::aie::vector<T, N> mac(const ::aie::vector<T, N>& acc,
                                          bool sub) const {
    decltype(::aie::mul(lhs, rhs)) a;
    a.from_vector(acc);
    a = sub ? ::aie::msc(a, lhs, rhs) : ::aie::mac(a, lhs, rhs);
    return a.template to_vector<T>();
  }
};

}  // namespace internal

template <typename T, int N>
struct vec_t {
  static constexpr int length = N;
  static constexpr int width = widthof<T>() * N;
  using value_type = T;
  using vector_type = ::aie::vector<T, N>;

  vector_type v;

  vec_t() = default;
  TAPA_AIE_INLINE vec_t(const vector_type& x) : v(x) {}
  TAPA_AIE_INLINE vec_t(const internal::vec_product<T, N>& p) : v(p.eval()) {}
  TAPA_AIE_INLINE operator vector_type() const { return v; }

  TAPA_AIE_INLINE T operator[](int pos) const { return v.get(pos); }
  TAPA_AIE_INLINE auto operator[](int pos) { return v[pos]; }
  TAPA_AIE_INLINE T get(int pos) const { return v.get(pos); }
  TAPA_AIE_INLINE void set(int pos, T value) { v.set(value, pos); }
  TAPA_AIE_INLINE void set(T value) { v = ::aie::broadcast<T, N>(value); }
  TAPA_AIE_INLINE vec_t& operator=(T value) {
    set(value);
    return *this;
  }

  friend TAPA_AIE_INLINE vec_t operator+(const vec_t& a, const vec_t& b) {
    return ::aie::add(a.v, b.v);
  }
  friend TAPA_AIE_INLINE vec_t operator-(const vec_t& a, const vec_t& b) {
    return ::aie::sub(a.v, b.v);
  }
  friend TAPA_AIE_INLINE vec_t operator+(const vec_t& a, T b) {
    return ::aie::add(a.v, b);
  }
  friend TAPA_AIE_INLINE vec_t operator-(const vec_t& a, T b) {
    return ::aie::sub(a.v, b);
  }
  friend TAPA_AIE_INLINE internal::vec_product<T, N> operator*(const vec_t& a,
                                                               const vec_t& b) {
    return {a.v, b.v};
  }
  friend TAPA_AIE_INLINE internal::vec_product<T, N> operator*(const vec_t& a,
                                                               T b) {
    return {a.v, ::aie::broadcast<T, N>(b)};
  }
  friend TAPA_AIE_INLINE internal::vec_product<T, N> operator*(T a,
                                                               const vec_t& b) {
    return {::aie::broadcast<T, N>(a), b.v};
  }
  TAPA_AIE_INLINE vec_t operator-() const { return ::aie::neg(v); }

  TAPA_AIE_INLINE vec_t& operator+=(const vec_t& o) {
    return *this = *this + o;
  }
  TAPA_AIE_INLINE vec_t& operator-=(const vec_t& o) {
    return *this = *this - o;
  }
  TAPA_AIE_INLINE vec_t& operator*=(const vec_t& o) {
    return *this = *this * o;
  }
  TAPA_AIE_INLINE vec_t& operator+=(T o) { return *this = *this + o; }
  TAPA_AIE_INLINE vec_t& operator-=(T o) { return *this = *this - o; }
  TAPA_AIE_INLINE vec_t& operator*=(T o) { return *this = *this * o; }
  TAPA_AIE_INLINE vec_t& operator+=(const internal::vec_product<T, N>& p) {
    v = p.mac(v, /*sub=*/false);
    return *this;
  }
  TAPA_AIE_INLINE vec_t& operator-=(const internal::vec_product<T, N>& p) {
    v = p.mac(v, /*sub=*/true);
    return *this;
  }
};

template <int N, typename T>
TAPA_AIE_INLINE vec_t<T, N> make_vec(T value) {
  return ::aie::broadcast<T, N>(value);
}

template <typename T, int N>
TAPA_AIE_INLINE T sum(const vec_t<T, N>& vec) {
  return ::aie::reduce_add(vec.v);
}

template <typename T, int N>
TAPA_AIE_INLINE vec_t<T, N> max(const vec_t<T, N>& a, const vec_t<T, N>& b) {
  return ::aie::max(a.v, b.v);
}

template <typename T, int N>
TAPA_AIE_INLINE vec_t<T, N> min(const vec_t<T, N>& a, const vec_t<T, N>& b) {
  return ::aie::min(a.v, b.v);
}

}  // namespace tapa
