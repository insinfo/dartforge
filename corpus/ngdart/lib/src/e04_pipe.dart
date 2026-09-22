import 'package:ngdart/angular.dart';

/// `@Pipe`: o que o oficial gera no arquivo do próprio pipe.
@Pipe('e04')
class E04Pipe {
  String transform(String v) => v;
}
