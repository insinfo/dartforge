class S(final str);
class C(final super.str) extends S;
//            ^^^^^
// [diag.superInitializingDeclaringParameter] Declaring parameters can't be super parameters.
