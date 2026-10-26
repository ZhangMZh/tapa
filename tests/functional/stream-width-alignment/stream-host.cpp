#include "stream.h"

#include <iostream>

#include <gflags/gflags.h>

DEFINE_string(bitstream, "", "XO/ZIP path; empty runs software simulation");

Wide WideValue(int transaction, int i) {
  Wide value = transaction * 100 + i;
  value[64] = i % 2;
  value[511] = transaction % 2;
  value[543] = 1;
  return value;
}

Signed SignedValue(int transaction, int i) {
  return (i % 2 ? -1 : 1) * (transaction * 100 + i);
}

Small SmallValue(int transaction, int i) { return transaction * 50 + i * 17; }

int main(int argc, char** argv) {
  gflags::ParseCommandLineFlags(&argc, &argv, true);
  tapa::stream<Wide> wide_in("wide_in"), wide_out("wide_out");
  tapa::stream<Signed> signed_in("signed_in"), signed_out("signed_out");
  tapa::stream<Small> small_in("small_in"), small_out("small_out");
  bool failed = false;
  tapa::task()
      .invoke(
          [](tapa::ostream<Wide>& wide, tapa::ostream<Signed>& signed_values,
             tapa::ostream<Small>& small) {
            for (int transaction = 0; transaction < kTransactions;
                 ++transaction) {
              for (int i = 0; i < TransactionSize(transaction); ++i) {
                wide.write(WideValue(transaction, i));
                signed_values.write(SignedValue(transaction, i));
                small.write(SmallValue(transaction, i));
              }
              wide.close();
              signed_values.close();
              small.close();
            }
          },
          wide_in, signed_in, small_in)
      .invoke(StreamWidthAlignment, tapa::executable(FLAGS_bitstream), wide_in,
              signed_in, small_in, wide_out, signed_out, small_out)
      .invoke(
          [&](tapa::istream<Wide>& wide, tapa::istream<Signed>& signed_values,
              tapa::istream<Small>& small) {
            for (int transaction = 0; transaction < kTransactions;
                 ++transaction) {
              for (int i = 0; i < TransactionSize(transaction); ++i) {
                // Always drain all channels, even if an earlier value failed.
                failed |= wide.read() != WideValue(transaction, i);
                failed |= signed_values.read() != SignedValue(transaction, i);
                failed |= small.read() != SmallValue(transaction, i);
              }
              wide.open();
              signed_values.open();
              small.open();
            }
          },
          wide_out, signed_out, small_out);
  std::cout << (failed ? "FAIL\n" : "PASS\n");
  return failed;
}
