// Consome os `.template.dart` do corpus. O builder do ngdart é
// `is_optional: true`: sem alguém pedindo a saída, ele não roda.
import 'package:corpus_ngdart/src/a01_interpolacao.template.dart' as a01;
import 'package:corpus_ngdart/src/a02_texto_estatico.template.dart' as a02;
import 'package:corpus_ngdart/src/a03_interpolacao_int.template.dart' as a03;
import 'package:corpus_ngdart/src/a04_interpolacao_com_texto.template.dart' as a04;
import 'package:corpus_ngdart/src/a05_duas_interpolacoes.template.dart' as a05;
import 'package:corpus_ngdart/src/a06_acesso_a_propriedade.template.dart' as a06;
import 'package:corpus_ngdart/src/a07_ligacao_de_propriedade.template.dart' as a07;
import 'package:corpus_ngdart/src/a08_evento.template.dart' as a08;
import 'package:corpus_ngdart/src/a09_ng_if.template.dart' as a09;
import 'package:corpus_ngdart/src/a10_ng_for.template.dart' as a10;
import 'package:corpus_ngdart/src/a11_projecao.template.dart' as a11;
import 'package:corpus_ngdart/src/a12_projecao_com_irmaos.template.dart' as a12;
import 'package:corpus_ngdart/src/a13_componente_filho.template.dart' as a13;
import 'package:corpus_ngdart/src/a14_ligacoes_especiais.template.dart' as a14;
import 'package:corpus_ngdart/src/a15_referencia.template.dart' as a15;
import 'package:corpus_ngdart/src/a16_entrada_e_saida.template.dart' as a16;
import 'package:corpus_ngdart/src/a17_projecao_com_select.template.dart' as a17;
import 'package:corpus_ngdart/src/a18_interpolacao_nula.template.dart' as a18;
import 'package:corpus_ngdart/src/a19_atributo_sem_valor.template.dart' as a19;

void main() {
  print([
    a01.A01InterpolacaoNgFactory,
    a02.A02TextoEstaticoNgFactory,
    a03.A03InterpolacaoIntNgFactory,
    a04.A04InterpolacaoComTextoNgFactory,
    a05.A05DuasInterpolacoesNgFactory,
    a06.A06AcessoAPropriedadeNgFactory,
    a07.A07LigacaoDePropriedadeNgFactory,
    a08.A08EventoNgFactory,
    a09.A09NgIfNgFactory,
    a10.A10NgForNgFactory,
    a11.A11ProjecaoNgFactory,
    a12.A12ProjecaoComIrmaosNgFactory,
    a13.A13ComponenteFilhoNgFactory,
    a14.A14LigacoesEspeciaisNgFactory,
    a15.A15ReferenciaNgFactory,
    a16.A16EntradaESaidaNgFactory,
    a17.A17ProjecaoComSelectNgFactory,
    a18.A18InterpolacaoNulaNgFactory,
    a19.A19AtributoSemValorNgFactory,
  ].length);
}
