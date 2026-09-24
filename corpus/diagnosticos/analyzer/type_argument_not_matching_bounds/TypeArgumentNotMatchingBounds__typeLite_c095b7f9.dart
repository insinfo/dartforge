typedef Cb<T extends int> = void Function();
var t = Cb<String>;
//      ^^^^^^^^^^
// [context 1] The inverted type 'Cb<String>' is also not regular-bounded, so the type is not well-bounded.
//         ^^^^^^
// [diag.typeArgumentNotMatchingBounds][context 1] 'String' doesn't conform to the bound 'int' of the type parameter 'T'.
