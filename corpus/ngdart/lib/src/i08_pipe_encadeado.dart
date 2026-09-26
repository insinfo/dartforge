import 'package:ngdart/angular.dart';

/// Sonda: pipes com `|`, com argumento e encadeados.
@Component(
  selector: 'i08-pipe-encadeado',
  templateUrl: 'i08_pipe_encadeado.html',
  directives: [coreDirectives],
  pipes: [commonPipes],
)
class I08PipeEncadeado {
  String nome = 'x';
  DateTime quando = DateTime(2024);
}
