import 'dart:ffi';
typedef F = int Function(int);
T genericRef<T extends Struct>(Pointer<T> p) => p.ref;
class C<R extends int Function(int), T extends Function> {
  void f(Pointer<NativeFunction<F>> p, Pointer<NativeFunction<T>> q) {
    p.asFunction<R>();
    q.asFunction<F>();
    p.asFunction<F>();
  }
}
void g<T extends Struct, U extends NativeType>(Pointer<T> p, Array<T> a) {
  p.refWithFinalizer;
  p[1];
  p + 1;
  sizeOf<T>();
  sizeOf<Int8>();
  sizeOf<U>();
  Pointer.fromFunction<T Function()>(g);
}
