// Consome os `.template.dart` do corpus. O builder do ngdart é
// `is_optional: true`: sem alguém pedindo a saída, ele não roda.
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
import 'package:corpus_ngdart/src/a20_getter_mutavel.template.dart' as a20;
import 'package:corpus_ngdart/src/a21_imutabilidade_composta.template.dart' as a21;
import 'package:corpus_ngdart/src/a22_form_com_forms_directives.template.dart' as a22;
import 'package:corpus_ngdart/src/a23_interp_chamada.template.dart' as a23;
import 'package:corpus_ngdart/src/a24_interp_ternario_binario.template.dart' as a24;
import 'package:corpus_ngdart/src/a25_local_ancestral.template.dart' as a25;
import 'package:corpus_ngdart/src/a26_constantes_em_embutida.template.dart' as a26;
import 'package:corpus_ngdart/src/a27_on_push.template.dart' as a27;
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
import 'package:corpus_ngdart/src/b17_host_listener_com_args.template.dart' as b17;
import 'package:corpus_ngdart/src/b18_providers_vazio.template.dart' as b18;
import 'package:corpus_ngdart/src/b19_pipes_sem_uso.template.dart' as b19;
import 'package:corpus_ngdart/src/b20_view_child_dois.template.dart' as b20;
import 'package:corpus_ngdart/src/b21_host_listener_explicito.template.dart' as b21;
import 'package:corpus_ngdart/src/b22_estilo_csslib.template.dart' as b22;
import 'package:corpus_ngdart/src/b23_so_on_destroy.template.dart' as b23;
import 'package:corpus_ngdart/src/c01_ligacao_e_texto.template.dart' as c01;
import 'package:corpus_ngdart/src/c02_evento_e_ligacao.template.dart' as c02;
import 'package:corpus_ngdart/src/c03_dois_elementos_ligados.template.dart' as c03;
import 'package:corpus_ngdart/src/c04_evento_com_argumento.template.dart' as c04;
import 'package:corpus_ngdart/src/c05_expressoes.template.dart' as c05;
import 'package:corpus_ngdart/src/c06_interpolacao_em_cadeia.template.dart' as c06;
import 'package:corpus_ngdart/src/c07_atributo_interpolado.template.dart' as c07;
import 'package:corpus_ngdart/src/c08_eventos_irmaos.template.dart' as c08;
import 'package:corpus_ngdart/src/c09_pipe_na_interpolacao.template.dart' as c09;
import 'package:corpus_ngdart/src/c10_entrada_antes_da_propriedade.template.dart' as c10;
import 'package:corpus_ngdart/src/c11_ligacoes_constantes.template.dart' as c11;
import 'package:corpus_ngdart/src/c12_handler_atribuicao.template.dart' as c12;
import 'package:corpus_ngdart/src/c13_evento_em_ng_for.template.dart' as c13;
import 'package:corpus_ngdart/src/c14_keyup_enter.template.dart' as c14;
import 'package:corpus_ngdart/src/c16_atributo_interpolado.template.dart' as c16;
import 'package:corpus_ngdart/src/c17_pipe_com_argumento.template.dart' as c17;
import 'package:corpus_ngdart/src/c18_evento_no_projetado.template.dart' as c18;
import 'package:corpus_ngdart/src/d01_dois_filhos.template.dart' as d01;
import 'package:corpus_ngdart/src/d02_filho_aninhado.template.dart' as d02;
import 'package:corpus_ngdart/src/d03_filho_com_entrada.template.dart' as d03;
import 'package:corpus_ngdart/src/d04_projecao_no_filho.template.dart' as d04;
import 'package:corpus_ngdart/src/d05_filho_ciclo.template.dart' as d05;
import 'package:corpus_ngdart/src/d05_usa_ciclo.template.dart' as d05;
import 'package:corpus_ngdart/src/d06_filho_saida.template.dart' as d06;
import 'package:corpus_ngdart/src/d06_usa_saida.template.dart' as d06;
import 'package:corpus_ngdart/src/d07_filho_on_push.template.dart' as d07;
import 'package:corpus_ngdart/src/d07_usa_on_push.template.dart' as d07;
import 'package:corpus_ngdart/src/d08_filho_injetado.template.dart' as d08;
import 'package:corpus_ngdart/src/d08_usa_injetado.template.dart' as d08;
import 'package:corpus_ngdart/src/d09_projecao_select.template.dart' as d09;
import 'package:corpus_ngdart/src/d09_usa_projecao.template.dart' as d09;
import 'package:corpus_ngdart/src/d10_campo_var.template.dart' as d10;
import 'package:corpus_ngdart/src/d10_filho_var.template.dart' as d10;
import 'package:corpus_ngdart/src/f01_if_com_for.template.dart' as f01;
import 'package:corpus_ngdart/src/f02_dois_ifs_irmaos.template.dart' as f02;
import 'package:corpus_ngdart/src/g01_form_ng_model.template.dart' as g01;
import 'package:corpus_ngdart/src/g02_select_ng_model.template.dart' as g02;
import 'package:corpus_ngdart/src/h01_cabecalho.template.dart' as h01;
import 'package:corpus_ngdart/src/h01_usa_cabecalho.template.dart' as h01;
import 'package:corpus_ngdart/src/h02_campo.template.dart' as h02;
import 'package:corpus_ngdart/src/h02_usa_campo.template.dart' as h02;
import 'package:corpus_ngdart/src/h03_opcoes.template.dart' as h03;
import 'package:corpus_ngdart/src/i01_ng_for_index.template.dart' as i01;
import 'package:corpus_ngdart/src/i02_ng_for_track_by.template.dart' as i02;
import 'package:corpus_ngdart/src/i03_ng_for_first_last.template.dart' as i03;
import 'package:corpus_ngdart/src/i04_ng_switch.template.dart' as i04;
import 'package:corpus_ngdart/src/i05_estilo_unidade.template.dart' as i05;
import 'package:corpus_ngdart/src/i06_ref_em_evento.template.dart' as i06;
import 'package:corpus_ngdart/src/i07_ref_em_interpolacao.template.dart' as i07;
import 'package:corpus_ngdart/src/i08_pipe_encadeado.template.dart' as i08;
import 'package:corpus_ngdart/src/i09_ng_container.template.dart' as i09;
import 'package:corpus_ngdart/src/i10_template_explicito.template.dart' as i10;
import 'package:corpus_ngdart/src/i11_ng_class.template.dart' as i11;
import 'package:corpus_ngdart/src/i12_ng_style.template.dart' as i12;
import 'package:corpus_ngdart/src/i13_seguro_nulo.template.dart' as i13;
import 'package:corpus_ngdart/src/i14_inner_html.template.dart' as i14;
import 'package:corpus_ngdart/src/i15_view_children.template.dart' as i15;
import 'package:corpus_ngdart/src/i16_view_child_componente.template.dart' as i16;
import 'package:corpus_ngdart/src/i17_host_binding_componente.template.dart' as i17;
import 'package:corpus_ngdart/src/i18_host_listener_componente.template.dart' as i18;
import 'package:corpus_ngdart/src/i19_providers_classe.template.dart' as i19;
import 'package:corpus_ngdart/src/i20_i18n.template.dart' as i20;
import 'package:corpus_ngdart/src/i21_ng_container_for.template.dart' as i21;
import 'package:corpus_ngdart/src/i22_ng_for_objeto.template.dart' as i22;
import 'package:corpus_ngdart/src/i23_entrada_getter.template.dart' as i23;
import 'package:corpus_ngdart/src/i24_two_way_filho.template.dart' as i24;
import 'package:corpus_ngdart/src/i25_on_push_entrada.template.dart' as i25;
import 'package:corpus_ngdart/src/i26_ng_content_varios.template.dart' as i26;
import 'package:corpus_ngdart/src/i27_evento_evento.template.dart' as i27;
import 'package:corpus_ngdart/src/i28_attr_class.template.dart' as i28;
import 'package:corpus_ngdart/src/i29_async_pipe.template.dart' as i29;
import 'package:corpus_ngdart/src/i30_template_outlet.template.dart' as i30;
import 'package:corpus_ngdart/src/i31_ng_for_campo_indice.template.dart' as i31;
import 'package:corpus_ngdart/src/i32_evento_filho_ref.template.dart' as i32;
import 'package:corpus_ngdart/src/i33_ng_if_aninhado_em_for.template.dart' as i33;
import 'package:corpus_ngdart/src/i34_texto_ternario_nulo.template.dart' as i34;
import 'package:corpus_ngdart/src/i35_i18n_formas.template.dart' as i35;
import 'package:corpus_ngdart/src/i36_prefixos_longos.template.dart' as i36;
import 'package:corpus_ngdart/src/i37_filho_callback.template.dart' as i37;
import 'package:corpus_ngdart/src/i37_usa_callback.template.dart' as i37;
import 'package:corpus_ngdart/src/i38_estilo_tipos.template.dart' as i38;
import 'package:corpus_ngdart/src/i39_ng_container_formas.template.dart' as i39;
import 'package:corpus_ngdart/src/i40_ng_container_texto.template.dart' as i40;
import 'package:corpus_ngdart/src/i41_ng_class_formas.template.dart' as i41;
import 'package:corpus_ngdart/src/i42_ng_switch_formas.template.dart' as i42;
import 'package:corpus_ngdart/src/i43_micro_dois_pontos.template.dart' as i43;
import 'package:corpus_ngdart/src/i44_ref_em_embutida.template.dart' as i44;
import 'package:corpus_ngdart/src/i45_ref_formas.template.dart' as i45;
import 'package:corpus_ngdart/src/i46_view_child_projetado.template.dart' as i46;
import 'package:corpus_ngdart/src/i47_view_child_em_if.template.dart' as i47;
import 'package:corpus_ngdart/src/i48_ref_projetado_lido.template.dart' as i48;
import 'package:corpus_ngdart/src/i49_view_children_formas.template.dart' as i49;
import 'package:corpus_ngdart/src/i50_view_child_tipos.template.dart' as i50;
import 'package:corpus_ngdart/src/i51_view_child_dinamico.template.dart' as i51;
import 'package:corpus_ngdart/src/i52_ng_for_dinamico.template.dart' as i52;
import 'package:corpus_ngdart/src/i53_template_formas.template.dart' as i53;
import 'package:corpus_ngdart/src/i54_pipes_aninhados.template.dart' as i54;
import 'package:corpus_ngdart/src/i55_host_binding_formas.template.dart' as i55;
import 'package:corpus_ngdart/src/i56_usa_host_binding.template.dart' as i56;
import 'package:corpus_ngdart/src/i57_seguranca.template.dart' as i57;
import 'package:corpus_ngdart/src/i58_async_formas.template.dart' as i58;
import 'package:corpus_ngdart/src/i59_contador.template.dart' as i59;
import 'package:corpus_ngdart/src/i59_usa_contador.template.dart' as i59;
import 'package:corpus_ngdart/src/i60_provider_use_class.template.dart' as i60;
import 'package:corpus_ngdart/src/i61_provider_use_value.template.dart' as i61;
import 'package:corpus_ngdart/src/i62_provider_use_factory.template.dart' as i62;
import 'package:corpus_ngdart/src/i63_provider_use_existing.template.dart' as i63;
import 'package:corpus_ngdart/src/i64_provider_multi.template.dart' as i64;
import 'package:corpus_ngdart/src/i65_provider_listas.template.dart' as i65;
import 'package:corpus_ngdart/src/i66_provider_dependencias.template.dart' as i66;
import 'package:corpus_ngdart/src/i67_provider_externo.template.dart' as i67;
import 'package:corpus_ngdart/src/i68_content_child_formas.template.dart' as i68;
import 'package:corpus_ngdart/src/i69_providers_e_consulta.template.dart' as i69;
import 'package:corpus_ngdart/src/i70_provider_ansioso_externo.template.dart' as i70;
import 'package:corpus_ngdart/src/i71_provider_valores.template.dart' as i71;
import 'package:corpus_ngdart/src/i72_usa_provider.template.dart' as i72;
import 'package:corpus_ngdart/src/i73_usa_consulta_read.template.dart' as i73;

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
    a20.A20GetterMutavelNgFactory,
    a21.A21ImutabilidadeCompostaNgFactory,
    a22.A22FormComFormsDirectivesNgFactory,
    a23.A23InterpChamadaNgFactory,
    a24.A24InterpTernarioBinarioNgFactory,
    a25.A25LocalAncestralNgFactory,
    a26.A26ConstantesEmEmbutidaNgFactory,
    a27.A27OnPushNgFactory,
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
    b17.B17HostListenerComArgsNgFactory,
    b18.B18ProvidersVazioNgFactory,
    b19.B19PipesSemUsoNgFactory,
    b20.B20ViewChildDoisNgFactory,
    b21.B21HostListenerExplicitoNgFactory,
    b22.B22EstiloCsslibNgFactory,
    b23.B23SoOnDestroyNgFactory,
    c01.C01LigacaoETextoNgFactory,
    c02.C02EventoELigacaoNgFactory,
    c03.C03DoisElementosLigadosNgFactory,
    c04.C04EventoComArgumentoNgFactory,
    c05.C05ExpressoesNgFactory,
    c06.C06InterpolacaoEmCadeiaNgFactory,
    c07.C07AtributoInterpoladoNgFactory,
    c08.C08EventosIrmaosNgFactory,
    c09.C09PipeNaInterpolacaoNgFactory,
    c10.C10EntradaAntesDaPropriedadeNgFactory,
    c11.C11LigacoesConstantesNgFactory,
    c12.C12HandlerAtribuicaoNgFactory,
    c13.C13EventoEmNgForNgFactory,
    c14.C14KeyupEnterNgFactory,
    c16.C16AtributoInterpoladoNgFactory,
    c17.C17PipeComArgumentoNgFactory,
    c18.C18EventoNoProjetadoNgFactory,
    d01.D01DoisFilhosNgFactory,
    d02.D02FilhoAninhadoNgFactory,
    d03.D03FilhoComEntradaNgFactory,
    d04.D04ProjecaoNoFilhoNgFactory,
    d05.D05FilhoCicloNgFactory,
    d05.D05UsaCicloNgFactory,
    d06.D06FilhoSaidaNgFactory,
    d06.D06UsaSaidaNgFactory,
    d07.D07FilhoOnPushNgFactory,
    d07.D07UsaOnPushNgFactory,
    d08.D08FilhoInjetadoNgFactory,
    d08.D08UsaInjetadoNgFactory,
    d09.D09ProjecaoSelectNgFactory,
    d09.D09UsaProjecaoNgFactory,
    d10.D10CampoVarNgFactory,
    d10.D10FilhoVarNgFactory,
    f01.F01IfComForNgFactory,
    f02.F02DoisIfsIrmaosNgFactory,
    g01.G01FormNgModelNgFactory,
    g02.G02SelectNgModelNgFactory,
    h01.H01CabecalhoNgFactory,
    h01.H01UsaCabecalhoNgFactory,
    h02.H02CampoNgFactory,
    h02.H02UsaCampoNgFactory,
    h03.H03OpcoesNgFactory,
    i01.I01NgForIndexNgFactory,
    i02.I02NgForTrackByNgFactory,
    i03.I03NgForFirstLastNgFactory,
    i04.I04NgSwitchNgFactory,
    i05.I05EstiloUnidadeNgFactory,
    i06.I06RefEmEventoNgFactory,
    i07.I07RefEmInterpolacaoNgFactory,
    i08.I08PipeEncadeadoNgFactory,
    i09.I09NgContainerNgFactory,
    i10.I10TemplateExplicitoNgFactory,
    i11.I11NgClassNgFactory,
    i12.I12NgStyleNgFactory,
    i13.I13SeguroNuloNgFactory,
    i14.I14InnerHtmlNgFactory,
    i15.I15ViewChildrenNgFactory,
    i16.I16ViewChildComponenteNgFactory,
    i17.I17HostBindingComponenteNgFactory,
    i18.I18HostListenerComponenteNgFactory,
    i19.I19ProvidersClasseNgFactory,
    i20.I20I18nNgFactory,
    i21.I21NgContainerForNgFactory,
    i22.I22NgForObjetoNgFactory,
    i23.I23EntradaGetterNgFactory,
    i24.I24TwoWayFilhoNgFactory,
    i25.I25OnPushEntradaNgFactory,
    i26.I26NgContentVariosNgFactory,
    i27.I27EventoEventoNgFactory,
    i28.I28AttrClassNgFactory,
    i29.I29AsyncPipeNgFactory,
    i30.I30TemplateOutletNgFactory,
    i31.I31NgForCampoIndiceNgFactory,
    i32.I32EventoFilhoRefNgFactory,
    i33.I33NgIfAninhadoEmForNgFactory,
    i34.I34TextoTernarioNuloNgFactory,
    i35.I35I18nFormasNgFactory,
    i36.I36PrefixosLongosNgFactory,
    i37.I37FilhoCallbackNgFactory,
    i37.I37UsaCallbackNgFactory,
    i38.I38EstiloTiposNgFactory,
    i39.I39NgContainerFormasNgFactory,
    i40.I40NgContainerTextoNgFactory,
    i41.I41NgClassFormasNgFactory,
    i42.I42NgSwitchFormasNgFactory,
    i43.I43MicroDoisPontosNgFactory,
    i44.I44RefEmEmbutidaNgFactory,
    i45.I45RefFormasNgFactory,
    i46.I46ViewChildProjetadoNgFactory,
    i47.I47ViewChildEmIfNgFactory,
    i48.I48RefProjetadoLidoNgFactory,
    i49.I49ViewChildrenFormasNgFactory,
    i50.I50ViewChildTiposNgFactory,
    i51.I51ViewChildDinamicoNgFactory,
    i52.I52NgForDinamicoNgFactory,
    i53.I53TemplateFormasNgFactory,
    i54.I54PipesAninhadosNgFactory,
    i55.I55HostBindingFormasNgFactory,
    i56.I56UsaHostBindingNgFactory,
    i57.I57SegurancaNgFactory,
    i58.I58AsyncFormasNgFactory,
    i59.I59ContadorNgFactory,
    i59.I59UsaContadorNgFactory,
    i60.I60ProviderUseClassNgFactory,
    i61.I61ProviderUseValueNgFactory,
    i62.I62ProviderUseFactoryNgFactory,
    i63.I63ProviderUseExistingNgFactory,
    i64.I64ProviderMultiNgFactory,
    i65.I65ProviderListasNgFactory,
    i66.I66ProviderDependenciasNgFactory,
    i67.I67ProviderExternoNgFactory,
    i68.I68ContentChildFormasNgFactory,
    i69.I69ProvidersEConsultaNgFactory,
    i70.I70ProviderAnsiosoExternoNgFactory,
    i71.I71ProviderValoresNgFactory,
    i72.I72UsaProviderNgFactory,
    i73.I73UsaConsultaReadNgFactory,
  ].length);
}
