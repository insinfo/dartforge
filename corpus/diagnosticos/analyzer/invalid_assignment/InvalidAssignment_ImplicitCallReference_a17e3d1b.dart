class C {}

Function f = C();
//           ^^^
// [diag.invalidAssignment] A value of type 'C' can't be assigned to a variable of type 'Function'.
