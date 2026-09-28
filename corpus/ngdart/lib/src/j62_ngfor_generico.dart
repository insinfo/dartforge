import 'package:ngdart/angular.dart';

class J62Celula {
  String rotulo = 'c';
}

/// `*ngFor` sobre coleção de tipo genérico aninhado (o calendário do
/// `li-date-picker`): o local é `List<import.J62Celula>`, e o de dentro,
/// `J62Celula`.
@Component(
  selector: 'j62-ngfor-generico',
  templateUrl: 'j62_ngfor_generico.html',
  directives: [NgFor],
)
class J62NgforGenerico {
  List<List<J62Celula>> semanas = [];
  List<Map<String, int>> pares = [];
}
