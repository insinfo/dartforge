class S(var str);
class C(var super.str) extends S;
//          ^^^^^
// [diag.superInitializingDeclaringParameter] Declaring parameters can't be super parameters.
