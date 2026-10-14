import 'model.dart' as m;
int read(m.Outer o) { if (o case m.Outer(inner:m.C(value /* colon: keep */ :final captured))) { return captured; } return 0; }
