class const A() {
  late final int v = 0;
//^^^^
// [diag.lateFinalFieldWithConstConstructor] Can't have a late final field in a class with a generative const constructor.
  A.notConst() : this();
}
