import 'package:ngdart/angular.dart';

import 'i37_filho_callback.dart';

/// Sonda: método passado como valor a um filho e `trackBy` com `index`.
@Component(
  selector: 'i37-usa-callback',
  templateUrl: 'i37_usa_callback.html',
  directives: [coreDirectives, I37FilhoCallback],
)
class I37UsaCallback {
  List<String> itens = ['a'];
  void salvar() {}
  Object? rastrear(int i, dynamic x) => x;
}
