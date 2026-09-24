class A {}
class B extends A {}
class C extends D {}
class D {}

void f<X extends A, Y extends B>(X x) {
  if (x is Y) {
    D d = x;
//        ^
// [diag.invalidAssignment] A value of type 'X & Y' can't be assigned to a variable of type 'D'.
    print(d);
  }
}
