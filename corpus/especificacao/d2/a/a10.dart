abstract class Abs {
  const Abs();
}
mixin Mx {}
enum En { v; const En(); }
typedef TA = NaoC;
class NaoC {}
var a = const Abs();
var b = const Mx();
var c = const En();
var d = const TA();
var e = const TA.x();
var f = const Indef();
var g = const Indef.x();
