class C {
  final double d;
  const C(this.d);
}
class D extends C {
  const D(d) : super(d);
//      ^
// [context 1] The evaluated constructor 'C.new' is called by 'D.new' and 'D.new' is defined here.
//                   ^
// [context 2] The exception is 'A value of type 'String' can't be assigned to a parameter of type 'double' in a const constructor.' and occurs here.
}
const f = const D('0.0');
//        ^^^^^^^^^^^^^^
// [diag.constEvalThrowsException][context 1][context 2] Evaluation of this constant expression throws an exception.
