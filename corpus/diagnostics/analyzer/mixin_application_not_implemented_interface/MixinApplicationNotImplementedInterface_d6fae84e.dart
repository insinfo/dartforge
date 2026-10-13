class A<T> {}

mixin M on A<int> {}

class X = A<double> with M;
//                       ^
// [diag.mixinApplicationNotImplementedInterface] 'M' can't be mixed onto 'A<double>' because 'A<double>' doesn't implement 'A<int>'.
