abstract class MyStream<T> extends Stream<T> {
  factory MyStream() => throw 0;
}
f(MyStream<dynamic> e) async {
  await for (var id in e) {
    id;
  }
}
