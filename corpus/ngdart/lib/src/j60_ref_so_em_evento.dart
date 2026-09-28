import 'package:ngdart/angular.dart';

/// `#ref` lido só num handler de evento, seguido de elementos com ligação
/// (o `<select #campo (change)=...>` do lookup de processos do limitless_ui):
/// o campo do elemento só entra na classe depois dos elementos ligados na
/// detecção.
@Component(
  selector: 'j60-ref-so-em-evento',
  templateUrl: 'j60_ref_so_em_evento.html',
)
class J60RefSoEmEvento {
  String titulo = 't';
  String ativo = 'a';
  String? escolhido;

  void escolher(String? valor) {
    escolhido = valor;
  }
}
