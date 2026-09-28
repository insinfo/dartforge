import 'package:ngdart/angular.dart';

class J90Servico {}

class J90Outro {
  J90Outro(this.servico);

  final J90Servico servico;
}

@Directive(
  selector: '[j90-marca]',
  providers: [ClassProvider(J90Servico), ClassProvider(J90Outro)],
)
class J90Marca {}

/// `read:` de um provedor de diretiva numa consulta estática: o provedor
/// lido fica ansioso (`queriedTokens`) e a consulta recebe a instância.
@Component(
  selector: 'j90-leitura-de-provedor',
  template: '<span j90-marca #a></span><b j90-marca #b></b><i j90-marca #b></i>',
  directives: [J90Marca],
)
class J90LeituraDeProvedor {
  @ViewChild('a', read: J90Outro)
  J90Outro? outro;

  @ViewChildren('b', read: J90Servico)
  List<J90Servico>? servicos;

  @ViewChild('b', read: J90Marca)
  J90Marca? primeira;
}
