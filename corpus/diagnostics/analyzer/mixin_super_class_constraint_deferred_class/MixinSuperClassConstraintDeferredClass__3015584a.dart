import 'dart:math' deferred as math;
mixin M on math.Random {}
//         ^^^^^^^^^^^
// [diag.mixinSuperClassConstraintDeferredClass] Deferred classes can't be used as superclass constraints.
