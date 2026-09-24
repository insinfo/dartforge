class A {}
mixin M {}
class B<F extends num> = A<F> with M;
//                       ^^^^
// [diag.wrongNumberOfTypeArguments] The type 'A' is declared with 0 type parameters, but 1 type arguments were given.
