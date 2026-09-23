class A {
  late final int v = 0;
//^^^^
// [diag.lateFinalFieldWithConstConstructor] Can't have a late final field in a class with a generative const constructor.
  const A();
  A.notConst();
}
