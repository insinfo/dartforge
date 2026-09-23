// R-UP-09: UP de tipos de extensão: mesma declaração → argumento a argumento;
// sem `implements`, o único supertipo comum com outro tipo é Object?.
extension type E<T>(T x) {}

extension type F(int x) implements int {}

void main(List<String> args) {
  var b = args.isEmpty;
  var p = /*@*/b ? E<int>(1) : E<double>(1.5);
  var q = /*@*/b ? E<int>(1) : 1;
  var r = /*@*/b ? F(1) : 2;
  print([p, q, r]);
}