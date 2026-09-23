import 'package:ngdart/angular.dart';

import 'e04_pipe.dart';

/// Pipes com argumento, em atributo interpolado, em visão embutida aninhada
/// e um pipe do próprio pacote (import relativo). Os proxies são numerados
/// na ordem do `bindView`: o `date` depois do `*ngIf` é o `_pipe_date_0_1`,
/// e os de dentro do `*ngFor` leem a instância por duas `parentView`.
@Component(
  selector: 'c17-pipe-com-argumento',
  templateUrl: 'c17_pipe_com_argumento.html',
  directives: [coreDirectives],
  pipes: [commonPipes, E04Pipe],
)
class C17PipeComArgumento {
  DateTime quando = DateTime(2024);
  String nome = 'x';
  List<String> itens = ['a', 'b'];
  bool mostra = true;
}
