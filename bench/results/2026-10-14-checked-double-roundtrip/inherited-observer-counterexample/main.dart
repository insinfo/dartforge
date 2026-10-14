@pragma('vm:never-inline')
Object roundTrip(dynamic value) {
  final double scalar = value;
  return scalar;
}
void main() {
  dynamic original = 1.25;
  dynamic returned = roundTrip(original);
  print(original.missingMethod() == returned.missingMethod());
}
