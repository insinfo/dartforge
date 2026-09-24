void f() {
  (<T extends num>() {})<String>();
//                       ^^^^^^
// [diag.typeArgumentNotMatchingBounds] 'String' doesn't conform to the bound 'num' of the type parameter 'T'.
}
