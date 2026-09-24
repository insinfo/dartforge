class C {
  C operator-() => this;
}
extension E on C {
  C get negated => -super;
//                  ^^^^^
// [diag.superInExtension] The 'super' keyword can't be used in an extension because an extension doesn't have a superclass.
}
