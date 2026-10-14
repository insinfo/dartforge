// Native-only extension: upstream behavior after close is undefined.
import 'dart:ffi';

final class Word extends Struct {
  @Int64()
  external int value;
}

final class Triple extends Struct {
  @Int64()
  external int first;
  @Int64()
  external int second;
  @Int64()
  external int third;
}

Word unexpectedWord() => throw StateError('Closed word callback entered Dart');
Triple unexpectedTriple() => throw StateError('Closed triple callback entered Dart');

void main() {
  if (!const bool.fromEnvironment('dartforge.compiler')) {
    print('Late composite callback extension requires DartForge AOT');
    return;
  }
  final word = NativeCallable<Word Function()>.isolateLocal(unexpectedWord);
  final wordInvoke = word.nativeFunction.asFunction<Word Function()>();
  word.close();
  final small = wordInvoke();
  if (small.value != 0) throw StateError('Closed direct return was not zero');
  final triple = NativeCallable<Triple Function()>.isolateLocal(unexpectedTriple);
  final tripleInvoke = triple.nativeFunction.asFunction<Triple Function()>();
  triple.close();
  final large = tripleInvoke();
  if (large.first != 0 || large.second != 0 || large.third != 0) {
    throw StateError('Closed indirect return was not zero');
  }
  print('closed composite direct and indirect native returns zeroed');
}
