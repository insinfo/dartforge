import 'cycle_b.dart';
int aValue() { return 1; }
int cycleValue() { return aValue() + bValue(); }
