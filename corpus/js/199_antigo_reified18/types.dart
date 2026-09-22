abstract class Service {
  int value();
}

class ConcreteService implements Service {
  int value() => 42;
}

int read<T extends Service>(T service) => service.value();
bool accepts<T>(Object? value) => value is T;
T checked<T>(Object? value) => value as T;
bool nested<T>(Object? value) => accepts<List<T>>(value);
bool Function(Object?) predicate<T>() => (Object? value) => value is T;
T identity<T>(T value) => value;
T? maybe<T extends Object>(T? value) => value;

