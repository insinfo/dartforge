class A {
  A({num a = 1.2});
}
class B extends A{
  B({int super.a});
//             ^
// [diag.missingDefaultValueForParameter] The parameter 'a' can't have a value of 'null' because of its type, but the implicit default value is 'null'.
}
