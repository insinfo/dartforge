// Consome os `.template.dart` do corpus. O builder do ngdart Ã©
// `is_optional: true`: sem alguÃ©m pedindo a saÃ­da, ele nÃ£o roda.
// Gerado por scripts/corpus-ngdart.ps1.
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
import 'package:corpus_ngdart/src/b01_ciclo_de_vida.template.dart' as b01;
import 'package:corpus_ngdart/src/b02_providers.template.dart' as b02;
import 'package:corpus_ngdart/src/b03_view_child.template.dart' as b03;
import 'package:corpus_ngdart/src/b04_host_listener.template.dart' as b04;
import 'package:corpus_ngdart/src/b05_host_binding.template.dart' as b05;
import 'package:corpus_ngdart/src/b06_encapsulation.template.dart' as b06;
import 'package:corpus_ngdart/src/b07_estilo.template.dart' as b07;
import 'package:corpus_ngdart/src/b08_after_changes.template.dart' as b08;
import 'package:corpus_ngdart/src/b09_after_view_init.template.dart' as b09;
import 'package:corpus_ngdart/src/b10_after_view_checked.template.dart' as b10;
import 'package:corpus_ngdart/src/b11_after_content_init.template.dart' as b11;
import 'package:corpus_ngdart/src/b12_do_check.template.dart' as b12;
import 'package:corpus_ngdart/src/b13_after_content_checked.template.dart' as b13;
import 'package:corpus_ngdart/src/b14_ciclo_completo.template.dart' as b14;
import 'package:corpus_ngdart/src/b15_estilo_rico.template.dart' as b15;
import 'package:corpus_ngdart/src/b16_estilo_formas.template.dart' as b16;
import 'package:corpus_ngdart/src/c01_ligacao_e_texto.template.dart' as c01;
import 'package:corpus_ngdart/src/c02_evento_e_ligacao.template.dart' as c02;
import 'package:corpus_ngdart/src/c03_dois_elementos_ligados.template.dart' as c03;
import 'package:corpus_ngdart/src/c04_evento_com_argumento.template.dart' as c04;
import 'package:corpus_ngdart/src/c05_expressoes.template.dart' as c05;
import 'package:corpus_ngdart/src/c06_interpolacao_em_cadeia.template.dart' as c06;
import 'package:corpus_ngdart/src/c07_atributo_interpolado.template.dart' as c07;
import 'package:corpus_ngdart/src/d01_dois_filhos.template.dart' as d01;
import 'package:corpus_ngdart/src/d02_filho_aninhado.template.dart' as d02;
import 'package:corpus_ngdart/src/d03_filho_com_entrada.template.dart' as d03;
import 'package:corpus_ngdart/src/d04_projecao_no_filho.template.dart' as d04;

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
    b01.B01CicloDeVidaNgFactory,
    b02.B02ProvidersNgFactory,
    b03.B03ViewChildNgFactory,
    b04.B04HostListenerNgFactory,
    b05.B05HostBindingNgFactory,
    b06.B06EncapsulationNgFactory,
    b07.B07EstiloNgFactory,
    b08.B08AfterChangesNgFactory,
    b09.B09AfterViewInitNgFactory,
    b10.B10AfterViewCheckedNgFactory,
    b11.B11AfterContentInitNgFactory,
    b12.B12DoCheckNgFactory,
    b13.B13AfterContentCheckedNgFactory,
    b14.B14CicloCompletoNgFactory,
    b15.B15EstiloRicoNgFactory,
    b16.B16EstiloFormasNgFactory,
    c01.C01LigacaoETextoNgFactory,
    c02.C02EventoELigacaoNgFactory,
    c03.C03DoisElementosLigadosNgFactory,
    c04.C04EventoComArgumentoNgFactory,
    c05.C05ExpressoesNgFactory,
    c06.C06InterpolacaoEmCadeiaNgFactory,
    c07.C07AtributoInterpoladoNgFactory,
    d01.D01DoisFilhosNgFactory,
    d02.D02FilhoAninhadoNgFactory,
    d03.D03FilhoComEntradaNgFactory,
    d04.D04ProjecaoNoFilhoNgFactory,
  ].length);
}
