import 'model.dart' show threshold;
import 'model.dart' as a;
import 'other.dart' as b;
int read(int x) => switch (x) { threshold => a.threshold + b.threshold, _ => 0 };
int binding(int x) { if (x case final threshold) { return threshold; } return 0; }
