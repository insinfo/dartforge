import 'dart:math' deferred as math;
mixin M implements math.Random {}
//                 ^^^^^^^^^^^
// [diag.implementsDeferredClass] Classes and mixins can't implement deferred classes.
