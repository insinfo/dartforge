enum E {
  v;
  late final f = this;
//^^^^
// [diag.lateFinalFieldWithConstConstructor] Can't have a late final field in a class with a generative const constructor.
}
