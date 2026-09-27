import 'package:ngdart/angular.dart';

/// Sonda: `#ref` repetido em visões diferentes e `let` que sombreia um
/// `#ref`.
@Component(
  selector: 'i95-ref-sombreado',
  templateUrl: 'i95_ref_sombreado.html',
  directives: [coreDirectives],
)
class I95RefSombreado {
  bool mostrar = true;
  List<String> itens = ['a'];
}
