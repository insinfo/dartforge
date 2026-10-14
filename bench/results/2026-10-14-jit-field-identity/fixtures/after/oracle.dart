import 'child.dart'; final instance = Child(); int turns = 0; void beforeReload() { if (++turns != 1 || instance.baseValue != 11 || instance.childValue != 22) throw StateError('Initial fields'); print(instance.baseValue + instance.childValue); instance.setBaseValue(101); instance.setChildValue(202); }

void afterReload() { if (++turns != 2 || instance.baseValue != 101 || instance.childValue != 202 || instance.extra != null) throw StateError('Live private fields were not preserved'); print(instance.baseValue + instance.childValue); }

void main() { beforeReload(); afterReload(); }
