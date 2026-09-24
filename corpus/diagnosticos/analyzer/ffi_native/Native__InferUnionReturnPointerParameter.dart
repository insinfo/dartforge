import 'dart:ffi';

@Native()
external MyUnion foo(Pointer x);

final class MyUnion extends Union {
  @Int8()
  external int a;
  @Int8()
  external int b;
}
