import 'package:ngdart/angular.dart';

/// Template na anotação em strings adjacentes e com escape (`\'`): as
/// posições do `REF` são as do valor da string, não as do arquivo.
@Component(
  selector: 'j63-template-concatenado',
  template: '<p [title]="nome">{{nome}}</p>'
      '<span [title]="rotulos[\'a\']">{{total}}</span>\n'
      "<b>{{ nome }}</b>",
)
class J63TemplateConcatenado {
  String nome = 'n';
  int total = 1;
  Map<String, String> rotulos = {};
}

/// String única com escape: o `REF` conta a partir do conteúdo, com as
/// posições do valor já decodificado.
@Component(
  selector: 'j63-escapado',
  template: '<p [title]="rotulos[\'a\']">{{nome}}</p>',
)
class J63Escapado {
  String nome = 'n';
  Map<String, String> rotulos = {};
}

/// String crua (`r'...'`).
@Component(
  selector: 'j63-cru',
  template: r'<p [title]="nome">{{nome}}</p>',
)
class J63Cru {
  String nome = 'n';
}
