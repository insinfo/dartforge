enum E {
  v;
  final int x = 0;
  const E();
  factory E._(this.x) => throw 0;
//            ^^^^^^
// [diag.fieldInitializerFactoryConstructor] Initializing formal parameters can't be used in factory constructors.
}

void f() {
  E._(0);
}
