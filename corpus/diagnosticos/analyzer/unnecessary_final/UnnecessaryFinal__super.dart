// %before-language-feature: primary-constructors
class A {
  A(this.value);
  int value;
}

class B extends A {
  B(final super.value);
//  ^^^^^
// [diag.unnecessaryFinal] The keyword 'final' isn't necessary because the parameter is implicitly 'final'.
}
