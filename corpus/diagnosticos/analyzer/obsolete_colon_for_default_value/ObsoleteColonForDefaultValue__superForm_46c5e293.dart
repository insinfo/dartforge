class A {
  String? a;
  A({this.a});
}

class B extends A {
  B({super.a : ''});
//           ^
// [diag.obsoleteColonForDefaultValue] Using a colon as the separator before a default value is no longer supported.
}
