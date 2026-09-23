class A {}

typedef T F<T>();

class C<T extends A> {
  C(F<T> f);
}

class NotA {}
NotA myF() => null;
//            ^^^^
// [diag.returnOfInvalidTypeFromFunction] A value of type 'Null' can't be returned from the function 'myF' because it has a return type of 'NotA'.

main() {
  var x = C(myF);
//    ^
// [diag.unusedLocalVariable] The value of the local variable 'x' isn't used.
//          ^^^
// [diag.argumentTypeNotAssignable] The argument type 'NotA Function()' can't be assigned to the parameter type 'F<A>'.
}
