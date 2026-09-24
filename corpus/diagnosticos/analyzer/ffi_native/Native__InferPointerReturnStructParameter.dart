import 'dart:ffi';

@Native()
external Pointer foo(MyStruct x);

final class MyStruct extends Struct {
  @Int8()
  external int value;
}
