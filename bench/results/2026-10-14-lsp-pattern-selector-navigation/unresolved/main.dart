import 'model.dart';
int value = 7;
int read(C c) { if (c case C(value:final captured)) { return captured; } return 0; }
