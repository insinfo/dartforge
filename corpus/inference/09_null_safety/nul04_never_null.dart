// R-NUL-04: `throw` tem tipo Never; `null` tem tipo Null.
void f(bool b) {
  var x = /*@*/b ? 1 : throw 0;
  var y = /*@*/null;
  var z = /*@*/[null];
  print([x, y, z]);
}

void main() => f(true);
