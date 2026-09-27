import 'package:ngdart/angular.dart';

/// `#ref` por visão: o mesmo nome em três níveis (cada visão lê o seu), o
/// de fora lido duas visões abaixo, um só lido na embutida, e um `let` que
/// esconde o de fora num evento.
@Component(
  selector: 'j11-ref-escopo',
  templateUrl: 'j11_ref_escopo.html',
  directives: [coreDirectives],
)
class J11RefEscopo {
  bool a = true;
  bool b = true;
  List<String> itens = ['x', 'y'];
  String visto = '';

  void ver(Object? v) {
    visto = '$v';
  }
}
