class A {
  String? a;
  A({this.a});
}

class B extends A {
  B({super.a : ''});
//           ^
// [diag.deprecatedColonForDefaultValue] Using a colon as the separator before a default value is deprecated and will not be supported in language version 3.0 and later.
}
