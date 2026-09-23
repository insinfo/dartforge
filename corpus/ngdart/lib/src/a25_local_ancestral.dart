import 'package:ngdart/angular.dart';

class Grupo {
  String nome = 'g';
  List<String> itens = ['a'];
}

/// Local de visão ancestral: a visão aninhada o lê pela cadeia de
/// `parentView`, com o cast para a classe da visão que o declara
/// (`getPropertyInView`) — na detecção e no handler de evento.
@Component(
  selector: 'a25-local-ancestral',
  templateUrl: 'a25_local_ancestral.html',
  directives: [coreDirectives],
)
class A25LocalAncestral {
  List<Grupo> grupos = [Grupo()];
  bool mostrar = true;
  void escolher(Grupo g, String item) {}
}
