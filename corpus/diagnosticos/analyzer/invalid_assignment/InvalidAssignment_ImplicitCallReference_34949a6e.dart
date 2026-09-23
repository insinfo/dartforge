class I {}
class J {}
enum E implements J {
  v
}
I x = E.v;
//    ^^^
// [diag.invalidAssignment] A value of type 'E' can't be assigned to a variable of type 'I'.
