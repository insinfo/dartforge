class C {
  factory C() sync* {
//            ^^^^
// [diag.nonSyncFactory] Factory bodies can't use 'async', 'async*', or 'sync*'.
  }
  C.named();
}
