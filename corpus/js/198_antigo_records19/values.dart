(String, int) user() => ('Gabriel', 28);
({String name, int age}) namedUser() => (age: 28, name: 'Gabriel');
T identity<T>(T value) => value;
bool accepts<T>(Object? value) => value is T;
(T, {T other}) pair<T>(T value) => (value, other: value);
int effect(int value) { print(value); return value; }

