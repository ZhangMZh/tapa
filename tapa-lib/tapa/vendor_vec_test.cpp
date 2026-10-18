#include "tapa/xilinx/hls/vec.h"

#include <gtest/gtest.h>

namespace tapa {
namespace {

// The Xilinx target needs its own translation unit, like
// vendor_stream_test.cpp, because vendor_parity_test.cpp includes the host
// vec_t definition first.
TEST(VendorVecTest, ScalarCompoundAssignmentCapturesAnAliasedRangeProxy) {
  vec_t<ap_uint<8>, 4> v;
  for (int i = 0; i < 4; ++i) v.set(i, ap_uint<8>(i + 1));

  v += v[0](7, 0);

  for (int i = 0; i < 4; ++i) EXPECT_EQ(v[i].to_int(), i + 2);
}

TEST(VendorVecTest, ScalarCompoundAssignmentCapturesAnAliasedBitProxy) {
  vec_t<ap_uint<8>, 4> v;
  v.set(0, ap_uint<8>(0b1111));
  for (int i = 1; i < 4; ++i) v.set(i, ap_uint<8>(1));

  // Bit 3 is initially one, but the first write turns it into zero.
  v += v[0][3];

  EXPECT_EQ(v[0].to_int(), 0b10000);
  for (int i = 1; i < 4; ++i) EXPECT_EQ(v[i].to_int(), 2);
}

TEST(VendorVecTest, ScalarCompoundAssignmentKeepsTheScalarsOwnWidth) {
  vec_t<ap_uint<8>, 2> v;
  v.set(0, ap_uint<8>(250));
  v.set(1, ap_uint<8>(100));

  // Casting the scalar to ap_uint<8> would divide by 44 instead of 300.
  v /= 300;

  EXPECT_EQ(v[0].to_int(), 0);
  EXPECT_EQ(v[1].to_int(), 0);
}

}  // namespace
}  // namespace tapa
