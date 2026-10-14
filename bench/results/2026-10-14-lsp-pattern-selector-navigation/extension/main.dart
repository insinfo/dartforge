import 'model.dart';
int read(C c) { if (c case C(value:final captured)) { return captured; } return 0; }
