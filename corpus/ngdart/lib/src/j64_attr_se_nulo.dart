import 'package:ngdart/angular.dart';

/// `[attr.x]` com `??` e com literal (o `data-sort-key` do `li-datatable`):
/// `setAttribute` quando a fonte não pode ser nula (`canBeNull`).
@Component(
  selector: 'j64-attr-se-nulo',
  templateUrl: 'j64_attr_se_nulo.html',
)
class J64AttrSeNulo {
  String? chave;
  int? limite;
  String? outra;
}
