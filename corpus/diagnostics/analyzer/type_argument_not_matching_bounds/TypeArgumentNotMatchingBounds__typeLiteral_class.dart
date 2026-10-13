class C<T extends int> {}
var t = C<String>;
//      ^^^^^^^^^
// [context 1] The inverted type 'C<String>' is also not regular-bounded, so the type is not well-bounded.
//        ^^^^^^
// [diag.typeArgumentNotMatchingBounds][context 1] 'String' doesn't conform to the bound 'int' of the type parameter 'T'.
