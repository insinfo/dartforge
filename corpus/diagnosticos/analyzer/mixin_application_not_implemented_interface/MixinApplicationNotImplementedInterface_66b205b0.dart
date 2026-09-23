abstract class A<T> {}
class B {}
mixin M<T> on A<T> {}
class C extends Object with M {}
//                          ^
// [diag.mixinApplicationNotImplementedInterface] 'M<dynamic>' can't be mixed onto 'Object' because 'Object' doesn't implement 'A<dynamic>'.
