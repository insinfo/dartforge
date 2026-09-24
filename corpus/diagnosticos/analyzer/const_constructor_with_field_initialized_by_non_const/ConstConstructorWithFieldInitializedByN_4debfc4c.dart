enum E {
  v;
  final int i = f();
//              ^^^
// [diag.constEvalMethodInvocation] Methods can't be invoked in constant expressions.
  const E();
//^^^^^
// [diag.constConstructorWithFieldInitializedByNonConst] Can't define the 'const' constructor because the field 'i' is initialized with a non-constant value.
}
int f() => 0;
