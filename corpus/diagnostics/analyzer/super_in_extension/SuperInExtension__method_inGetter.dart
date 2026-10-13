extension E on int {
  String get displayText => super.toString();
//                          ^^^^^
// [diag.superInExtension] The 'super' keyword can't be used in an extension because an extension doesn't have a superclass.
}
