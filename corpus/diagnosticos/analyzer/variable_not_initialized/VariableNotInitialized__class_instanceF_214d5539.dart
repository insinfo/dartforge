class A {
  final int v = 0;
  A() : v = 0;
//      ^
// [diag.fieldInitializedInInitializerAndDeclaration] Fields can't be initialized in the constructor if they are final and were already initialized at their declaration.
}
