class C {
  int operator[](int i) => 0;
}
extension E on C {
  int at(int i) => super[i];
//                 ^^^^^
// [diag.superInExtension] The 'super' keyword can't be used in an extension because an extension doesn't have a superclass.
}
