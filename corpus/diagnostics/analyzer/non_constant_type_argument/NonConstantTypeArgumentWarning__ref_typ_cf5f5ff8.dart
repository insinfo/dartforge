import 'dart:ffi';

T genericRef<T extends Struct>(Pointer<T> p) =>
    p.ref;
//  ^^^^^
// [diag.nonConstantTypeArgument] The type arguments to 'ref' must be known at compile time, so they can't be type parameters.
