final events = <String>[];
void record(List<int> output, int value) { events.add('record:$value'); output.add(value); }

List<int> canonical(int n) {
  final output = <int>[];
  for (var i = 0; i < n; i += 1) { record(output, i % 3); }
  return output;
}
List<int> prefix(int n) {
  final output = <int>[];
  for (int i = 0; i < n; ++i) { output.add(i % 7); }
  return output;
}
void JS(List<int> output, int value) { output.add(value); }
List<int> userNamedIntrinsic(int n) {
  final output = <int>[];
  for (var i = 0; i < n; i++) { JS(output, i % 11); }
  return output;
}
List<int> maximumDivisor(int n) {
  final output = <int>[];
  for (var i = 0; i < n; i++) { output.add(i % 1000000000); }
  return output;
}
List<int> negative(int n) {
  final output = <int>[];
  for (var i = -1; i < n; i++) { output.add(i % 3); }
  return output;
}
List<int> mutated(int n) {
  final output = <int>[];
  for (var i = 0; i < n; i++) { i += 1; output.add(i % 3); }
  return output;
}
List<int> shadowed(int n) {
  final output = <int>[];
  for (var i = 0; i < n; i++) { { final i = -1; output.add(i % 3); } }
  return output;
}
List<int> captured(int n) {
  final output = <int>[];
  for (var i = 0; i < n; i++) { output.add((() => i % 3)()); }
  return output;
}
List<int> callback(int n) {
  final output = <int>[];
  for (var i = 0; i < n; i++) { void local() { i += 1; } local(); output.add(i % 3); }
  return output;
}
List<int> nested(int n) {
  final output = <int>[];
  for (var i = 0; i < n; i++) { for (var j = 0; j < 1; j++) { output.add(i % 3); } }
  return output;
}
List<int> nonunit(int n) {
  final output = <int>[];
  for (var i = 0; i < n; i += 2) { output.add(i % 3); }
  return output;
}
List<num> negativeDivisor(int n) {
  final output = <num>[];
  for (var i = 0; i < n; i++) { output.add(i % -3); }
  return output;
}
List<int> oversizedDivisor(int n) {
  final output = <int>[];
  for (var i = 0; i < n; i++) { output.add(i % 1000000001); }
  return output;
}
List<num> dynamicDivisor(int n, dynamic divisor) {
  final output = <num>[];
  for (var i = 0; i < n; i++) { output.add(i % divisor); }
  return output;
}
class Bound {
  int get size { events.add('bound-get'); return 3; }
}
int boundCall() { events.add('bound-call'); return 3; }
List<int> getterBound(Bound bound) {
  final output = <int>[];
  for (var i = 0; i < bound.size; i++) { output.add(i % 5); }
  return output;
}
List<int> callBound() {
  final output = <int>[];
  for (var i = 0; i < boundCall(); i++) { output.add(i % 5); }
  return output;
}
List<int> conditionWrite() {
  final output = <int>[];
  for (var i = 0; i < ((i += 1) < 3 ? 3 : 0); i++) { output.add(i % 3); }
  return output;
}
List<int> conditionClosure(int n) {
  final output = <int>[];
  for (var i = 0; i < (() => n)(); i++) { output.add(i % 3); }
  return output;
}

void main() {
  print('canonical=${canonical(8)}');
  print('events=$events');
  print('prefix=${prefix(8)}');
  print('userNamedIntrinsic=${userNamedIntrinsic(4)}');
  print('maximumDivisor=${maximumDivisor(4)}');
  print('negative=${negative(3)}');
  print('mutated=${mutated(4)}');
  print('shadowed=${shadowed(3)}');
  print('captured=${captured(3)}');
  print('callback=${callback(4)}');
  print('nested=${nested(3)}');
  print('nonunit=${nonunit(5)}');
  print('negativeDivisor=${negativeDivisor(4)}');
  print('oversizedDivisor=${oversizedDivisor(4)}');
  print('dynamicDivisor=${dynamicDivisor(4, 3)}');
  try { dynamicDivisor(1, '3'); } catch (error) { print('dynamicError=${error is TypeError}'); }
  events.clear();
  print('getterBound=${getterBound(Bound())}:$events');
  events.clear();
  print('callBound=${callBound()}:$events');
  print('conditionWrite=${conditionWrite()}');
  print('conditionClosure=${conditionClosure(3)}');
}
