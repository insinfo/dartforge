class A {}
class B extends A {
  get g {
    return super.g;
//               ^
// [diag.undefinedSuperGetter] The getter 'g' isn't defined in a superclass of 'B'.
  }
}
