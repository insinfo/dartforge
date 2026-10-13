import 'dart:ffi';
@Native<Void Function()>()
external void nat();
void notNative() {}
final class S extends Struct {
  @Array(8)
  external Array<Void> a;
  @Array(8)
  external Array<Int8> b;
}
final class U extends Union {
  @Array(2)
  external Array<Handle> h;
}
int cb(int a) => a;
void use(Pointer<NativeFunction<Int32 Function(int)>> p, Pointer<NativeFunction<Int32 Function(Int32)>> q) {
  Native.addressOf<NativeFunction<Void Function()>>(nat);
  Native.addressOf<NativeFunction<Void Function()>>(notNative);
  Native.addressOf<NativeFunction<Void Function()>>(() {});
  Native.addressOf<NativeFunction<Void Function()>>('x');
  NativeCallable<Int32 Function(Int32)>.listener(cb);
  NativeCallable<Void Function(Int32)>.listener((int a) {});
  p.asFunction<int Function(int)>();
  q.asFunction<int Function(int)>();
}
