import 'dart:ffi';
final class S extends Struct {
  @Int32()
  external int x;
}
void f<T extends NativeType>(Pointer<T> p, Pointer<S> s) {
  p.ref;
  s.ref;
}
R g<R extends Function>(Pointer<NativeFunction<Int32 Function()>> p) {
  return p.asFunction<R>();
}
void h<T extends Struct>() {
  sizeOf<T>();
  Pointer<T> p = nullptr;
  p[0];
}
