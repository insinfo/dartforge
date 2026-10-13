final o = Object();
var v = [if (o) 'x'];
//           ^
// [diag.nonBoolCondition] Conditions must have a static type of 'bool'.
