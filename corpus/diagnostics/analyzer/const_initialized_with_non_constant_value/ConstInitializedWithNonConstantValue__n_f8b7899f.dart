class A {
  const A();
}
const a = new A();
//        ^^^^^^^
// [diag.constInitializedWithNonConstantValue] Const variables must be initialized with a constant value.
