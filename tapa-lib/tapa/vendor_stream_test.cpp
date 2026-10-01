#include "tapa/xilinx/hls/stream.h"

#include <gtest/gtest.h>

namespace tapa {
namespace {

// The HLS stream aliases conflict with the host stream types, so this lives
// in a separate translation unit of the existing vendor parity test.
TEST(VendorStreamTest, EmptyReadsReturnDefaultAfterDrainingATransaction) {
  tapa_stream<int> data_q;
  const auto& observer = data_q;
  EXPECT_TRUE(observer.empty());
  EXPECT_FALSE(observer.full());
  EXPECT_EQ(data_q.read(nullptr), 0);
  EXPECT_EQ(observer.peek(nullptr), 0);

  data_q.write(2333);
  EXPECT_FALSE(observer.empty());
  EXPECT_EQ(data_q.read(nullptr), 2333);
  EXPECT_TRUE(observer.empty());
  EXPECT_EQ(data_q.read(nullptr), 0);
  EXPECT_EQ(observer.peek(nullptr), 0);

  data_q.close();
  data_q.open();
  EXPECT_TRUE(observer.empty());
  EXPECT_EQ(data_q.read(nullptr), 0);
  EXPECT_EQ(observer.peek(nullptr), 0);
}

}  // namespace
}  // namespace tapa
