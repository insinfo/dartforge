import 'dart:ffi';

@Native()
external MyStruct foo();

final class MyStruct extends Struct {
  @Int8()
  external int value;
}
