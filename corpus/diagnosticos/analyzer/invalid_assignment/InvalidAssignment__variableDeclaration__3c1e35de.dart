class Point {
  final num x, y;
  Point(this.x, this.y);
  Point operator +(Point other) {
    return new Point(x+other.x, y+other.y);
  }
}
main() {
  var p1 = new Point(0, 0);
  var p2 = new Point(10, 10);
  int n = p1 + p2;
//        ^^^^^^^
// [diag.invalidAssignment] A value of type 'Point' can't be assigned to a variable of type 'int'.
  print(n);
}
