(int, {String label}) firstLocal() {
  const n = 1;
  return const (n, label: 'local');
}
(int, {String label}) secondLocal() {
  const n = 2;
  return const (n, label: 'local');
}
(int, {String label}) firstImplicit() {
  const n = 1;
  const value = (n, label: 'implicit');
  return value;
}
(int, {String label}) secondImplicit() {
  const n = 2;
  const value = (n, label: 'implicit');
  return value;
}
void main() {
  print('explicit=${firstLocal().$1}:${secondLocal().$1}:${firstLocal().$1}');
  print('implicit=${firstImplicit().$1}:${secondImplicit().$1}:${firstImplicit().$1}');
}
