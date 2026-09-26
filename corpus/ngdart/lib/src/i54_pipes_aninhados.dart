import 'package:ngdart/angular.dart';

/// Sonda: pipes aninhados em vários argumentos e em ligação.
@Component(
  selector: 'i54-pipes-aninhados',
  templateUrl: 'i54_pipes_aninhados.html',
  directives: [coreDirectives],
  pipes: [commonPipes],
)
class I54PipesAninhados {
  String nome = 'x';
  String fmt = 'y';
}
