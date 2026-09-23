import 'dart:ffi';

@Native()
external MyStruct foo(Pointer x);

final class MyStruct extends Struct {
  @Int8()
  external int value;
}
