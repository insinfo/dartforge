class Foo<T> {}
class Bar<T extends Foo<T>> {}
class Baz extends Bar {}
//                ^^^
// [context 1] The raw type was instantiated as 'Bar<Foo<dynamic>>', and is not regular-bounded.
// [diag.typeArgumentNotMatchingBounds][context 1] 'Foo<dynamic>' doesn't conform to the bound 'Foo<Foo<dynamic>>' of the type parameter 'T'.
void main() {}
