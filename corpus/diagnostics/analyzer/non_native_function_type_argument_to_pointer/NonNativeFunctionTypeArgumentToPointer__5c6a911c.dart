import 'dart:ffi';
typedef R = Int8 Function(Int8);
class C {
  void f(Pointer<Double> p) {
    p.asFunction<R>();
//    ^^^^^^^^^^
// [diag.undefinedMethod] The method 'asFunction' isn't defined for the type 'Pointer<Double>'.
  }
}
