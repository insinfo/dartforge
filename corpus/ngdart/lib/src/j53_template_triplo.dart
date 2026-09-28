import 'package:ngdart/angular.dart';

/// Template em aspas triplas com quebra de linha logo depois das aspas (o
/// `demo-page-breadcrumb` do limitless_ui): o Dart tira essa quebra do
/// valor; as posições do `REF` são as do `.dart`.
@Component(
  selector: 'j53-com-quebra',
  template: '''
<span [title]="nome">{{ nome }}</span>
<b (click)="nome = 'x'">b</b>
''',
)
class J53ComQuebra {
  String nome = 'n';
}

@Component(
  selector: 'j53-sem-quebra',
  template: """<i [title]="nome">{{nome}}</i>
<u>{{ nome }}</u>""",
)
class J53SemQuebra {
  String nome = 'n';
}
