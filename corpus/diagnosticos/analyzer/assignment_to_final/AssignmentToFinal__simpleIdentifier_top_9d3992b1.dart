int get x => 0;

void f() {
  x = 0;
//^
// [diag.assignmentToFinal] 'x' can't be used as a setter because it's final.
  x += 0;
//^
// [diag.assignmentToFinal] 'x' can't be used as a setter because it's final.
  ++x;
//  ^
// [diag.assignmentToFinal] 'x' can't be used as a setter because it's final.
  x++;
//^
// [diag.assignmentToFinal] 'x' can't be used as a setter because it's final.
}
