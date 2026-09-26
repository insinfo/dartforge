import 'package:ngdart/angular.dart';

/// Sonda: `#ref` da visão embutida e da visão do componente lidos na embutida.
@Component(
  selector: 'i44-ref-em-embutida',
  templateUrl: 'i44_ref_em_embutida.html',
  directives: [coreDirectives],
)
class I44RefEmEmbutida {
  List<String> itens = ['a'];
  void usar(Object? a, Object? b, String x) {}
}
