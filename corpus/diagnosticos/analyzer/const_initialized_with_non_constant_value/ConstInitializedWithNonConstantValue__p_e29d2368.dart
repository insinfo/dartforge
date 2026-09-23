class A {
  const A();
  int m() => 0;
}
final a = const A();
const c = a.m;
//        ^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
