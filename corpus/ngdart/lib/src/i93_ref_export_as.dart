import 'package:ngdart/angular.dart';

import 'i93_dica.dart';

/// Sonda: `#d="dica"` — a referência vale a diretiva exportada.
@Component(
  selector: 'i93-ref-export-as',
  templateUrl: 'i93_ref_export_as.html',
  directives: [I93Dica],
)
class I93RefExportAs {
  void usar(Object d) {}
}
