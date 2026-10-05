import 'dart:ffi';
final class S extends Struct {
  external Pointer<Void> p;
}
void g<T extends Struct>(Pointer<S> s, Allocator a, Pointer<T> p) {
  s.ref;
  (s).ref;
  (p).ref;
  a<S>();
  a<T>();
  Struct.create<T>();
  Pointer.fromFunction<Void Function()>(g, 1);
}
