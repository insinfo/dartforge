void foo<T extends num>(T a) {}
void bar() {
  foo<String>;
//    ^^^^^^
// [diag.typeArgumentNotMatchingBounds] 'String' doesn't conform to the bound 'num' of the type parameter 'T'.
}
