import 'package:ngdart/angular.dart';

class J71Config {}

class J71Externo {}

class J71Servico {
  J71Servico(@Optional() this.config, this.grupo);

  final J71Config? config;
  final J71Grupo grupo;
}

class J71Preguicoso {
  J71Preguicoso(this.externo);

  final J71Externo externo;
}

abstract class J71Rotulo {}

@Directive(selector: '[j71-grupo]')
class J71Grupo {}

/// Diretiva com `providers:` que não são só apelidos (o `LiScrollSpyDirective`
/// do limitless_ui): o serviço que ela injeta é criado antes dela, com a
/// dependência achada acima ou no injetor de fora; o que ninguém pede fica
/// preguiçoso; o apelido de um token de fora lê o injetor.
@Directive(
  selector: '[j71-espiao]',
  providers: [
    ClassProvider(J71Servico),
    ClassProvider(J71Preguicoso),
    ExistingProvider(J71Rotulo, J71Externo),
  ],
)
class J71Espiao {
  J71Espiao(this.servico);

  final J71Servico servico;
}

@Component(
  selector: 'j71-provedores-de-diretiva',
  templateUrl: 'j71_provedores_de_diretiva.html',
  directives: [J71Grupo, J71Espiao, NgIf],
  providers: [ClassProvider(J71Externo), ClassProvider(J71Config)],
)
class J71ProvedoresDeDiretiva {
  bool mostrar = true;
}
