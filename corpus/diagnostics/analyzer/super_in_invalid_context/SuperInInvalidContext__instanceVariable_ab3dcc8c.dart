class A {
  var a;
}
class B extends A {
 var b = super.a;
//       ^^^^^
// [diag.superInInvalidContext] Invalid context for 'super' invocation.
}
