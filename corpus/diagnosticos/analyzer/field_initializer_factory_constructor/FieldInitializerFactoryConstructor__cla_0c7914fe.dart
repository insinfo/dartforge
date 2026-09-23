class A {
  int x = 0;
  factory A(this.x) => throw 0;
//          ^^^^^^
// [diag.fieldInitializerFactoryConstructor] Initializing formal parameters can't be used in factory constructors.
}
