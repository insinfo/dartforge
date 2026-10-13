import 'dart:ffi';
typedef T = int Function(int);
class C<R extends int Function(int)> {
  void f(Pointer<NativeFunction<T>> p) {
    p.asFunction<R>();
//               ^
// [diag.nonConstantTypeArgument] The type arguments to 'asFunction' must be known at compile time, so they can't be type parameters.
  }
}
