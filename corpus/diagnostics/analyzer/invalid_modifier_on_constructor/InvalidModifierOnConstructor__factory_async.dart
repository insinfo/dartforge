class C {
  factory C() async {
//            ^^^^^
// [diag.nonSyncFactory] Factory bodies can't use 'async', 'async*', or 'sync*'.
    return C.named();
  }
  C.named();
}
