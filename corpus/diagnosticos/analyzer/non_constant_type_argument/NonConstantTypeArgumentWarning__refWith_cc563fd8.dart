import 'dart:ffi';

T genericRefWithFinalizer<T extends Struct>(Pointer<T> p) =>
    p.refWithFinalizer(nullptr);
//  ^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.nonConstantTypeArgument] The type arguments to 'refWithFinalizer' must be known at compile time, so they can't be type parameters.
