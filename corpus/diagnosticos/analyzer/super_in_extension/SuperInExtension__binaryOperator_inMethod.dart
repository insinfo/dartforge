extension E on int {
  int plusOne() => super + 1;
//                 ^^^^^
// [diag.superInExtension] The 'super' keyword can't be used in an extension because an extension doesn't have a superclass.
}
