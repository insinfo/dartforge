class A {}
class B extends A {
  f() {
    super.m = 0;
//        ^
// [diag.undefinedSuperSetter] The setter 'm' isn't defined in a superclass of 'B'.
  }
}
