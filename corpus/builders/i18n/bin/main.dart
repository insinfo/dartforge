import 'package:corpus_i18n/mensagens.i18n.dart';
import 'package:corpus_i18n/mensagens_en.i18n.dart';
import 'package:corpus_i18n/mensagens_pt_BR.i18n.dart';

void main() {
  for (final m in [Mensagens(), MensagensEn(), MensagensPtBR()]) {
    print('${m.locale} ${m.geral.titulo} ${m.geral.saudacao('Ana')}');
    print('${m.geral.numero} ${m.geral.ligado} ${m.geral.preco}');
    print('${m.menu.subMenu.fechar} ${m.geral.comEspacos} ${m.geral.comHifen}');
    print(m.geral.contagem(3));
    print(m.geral.exemplo.trim());
  }
  print(mensagensMap.length);
  print(mensagensEnMap['menu.abrir']);
}
