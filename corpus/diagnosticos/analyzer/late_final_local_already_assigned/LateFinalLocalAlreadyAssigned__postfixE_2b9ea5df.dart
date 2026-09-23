main() {
  late final int v = 0;
  v!;
// ^
// [diag.unnecessaryNonNullAssertion] The '!' will have no effect because the receiver can't be null.
  v;
}
