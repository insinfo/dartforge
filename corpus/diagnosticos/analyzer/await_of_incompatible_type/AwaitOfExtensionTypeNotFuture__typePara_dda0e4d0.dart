extension type A(Future<int> it) {}

void f<T extends A>(T a) async {
  await a;
//^^^^^
// [diag.awaitOfIncompatibleType] The 'await' expression can't be used for an expression with an extension type that is not a subtype of 'Future'.
}
