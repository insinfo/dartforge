import 'package:ngdart/angular.dart';

/// Pipe usado no template (`$pipe.uppercase(nome)`, a forma do ngdart 8 — o
/// `nome | uppercase` antigo nem compila no oficial). O gerador recusa.
@Component(
  selector: 'c09-pipe-na-interpolacao',
  templateUrl: 'c09_pipe_na_interpolacao.html',
  pipes: [commonPipes],
)
class C09PipeNaInterpolacao {
  String nome = 'x';
}