class A {
  int Function()? x;
  factory A(int this.x());
//          ^^^^^^^^^^^^
// [diag.fieldInitializerFactoryConstructor] Initializing formal parameters can't be used in factory constructors.
}
