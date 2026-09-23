import 'package:ngdart/angular.dart';

/// Handlers complexos (`_handleEvent_N`): atribuição, argumentos que não são
/// `$event`; e o tear-off (`(click)="salvar"`), que vira `salvar()` ou
/// `ir($event)` pelos parâmetros posicionais do método.
@Component(
  selector: 'c12-handler-atribuicao',
  templateUrl: 'c12_handler_atribuicao.html',
)
class C12HandlerAtribuicao {
  String valor = '';
  bool aberto = false;
  void salvar() {}
  void ir(Object? e) {}
  void mover(int a, String b) {}
}
