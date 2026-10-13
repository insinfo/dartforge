class const A() {
  late final int v;
//^^^^
// [diag.lateFinalFieldWithConstConstructor] Can't have a late final field in a class with a generative const constructor.
}
