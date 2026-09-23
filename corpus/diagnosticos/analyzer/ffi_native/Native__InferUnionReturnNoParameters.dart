import 'dart:ffi';

@Native()
external MyUnion foo();

final class MyUnion extends Union {
  @Int8()
  external int a;
  @Int8()
  external int b;
}
