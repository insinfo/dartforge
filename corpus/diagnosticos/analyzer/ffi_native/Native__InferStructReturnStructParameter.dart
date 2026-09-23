import 'dart:ffi';

@Native()
external MyStruct foo(MyStruct x);

final class MyStruct extends Struct {
  @Int8()
  external int value;
}
