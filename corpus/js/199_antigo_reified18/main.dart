// Convertido de tests/conformance/modules/reified18 (módulo antigo do corpus de conformidade).
import 'types.dart';

void main() {
  print(read(ConcreteService()));
  print(accepts<int>(7));
  print(accepts<String>(7));
  print(accepts<int?>(null));
  print(accepts<Object>(null));
  print(accepts<Object?>(null));
  print(checked<String>('checked'));
  print(identity<int?>(null));
  print(maybe<int>(null));
  print(identity('inferred'));
  var integers = <int>[1, 2];
  print(nested<int>(integers));
  print(nested<String>(integers));
  print(accepts<List<Object>>(integers));
  print(accepts<Iterable<int>>(integers));
  print(accepts<List<int>?>(null));
  print(accepts<List<int?>>(integers));
  var intPredicate = predicate<int>();
  var stringPredicate = predicate<String>();
  print(intPredicate(7));
  print(stringPredicate(7));
  print(stringPredicate('ok'));
  print(accepts<Service>(ConcreteService()));
  print(7 is! String);
  print(accepts<Iterable<int>>(integers.where((n) => n > 1)));
  print(accepts<List<int>>(integers.where((n) => n > 1).toList()));
  print(accepts<Iterable<String>>(integers.map((n) => 'mapped')));
  var nestedLists = [<int>[1], <String>['s']];
  print(accepts<List<List<Object>>>(nestedLists));
  print(accepts<List<List<int>>>(nestedLists));
}
