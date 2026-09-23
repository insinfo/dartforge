import 'dart:ffi';

@Native()
external MyUnion foo(MyStruct x);

final class MyStruct extends Struct {
  @Int8()
  external int value;
}

final class MyUnion extends Union {
  @Int8()
  external int a;
  @Int8()
  external int b;
}
