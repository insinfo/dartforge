// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i74_usa_provider_formas.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i74_usa_provider_formas.dart' as import1;
import 'i62_provider_use_factory.dart' as import2;
import 'dart:core';
import 'i63_provider_use_existing.dart' as import4;
import 'i64_provider_multi.dart' as import5;
import 'i60_provider_use_class.dart' as import6;
import 'i62_provider_use_factory.template.dart' as import7;
import 'i63_provider_use_existing.template.dart' as import8;
import 'i64_provider_multi.template.dart' as import9;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'i60_provider_use_class.template.dart' as import12;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import13;
import 'package:ngdart/src/core/linker/views/view.dart' as import14;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import15;
import 'package:ngdart/src/utilities.dart' as import16;
import 'dart:html' as import17;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import18;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import20;
import 'package:ngdart/src/meta/di_tokens.dart' as import21;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import23;
import 'i65_provider_listas.dart' as import24;
import 'i65_provider_listas.template.dart' as import25;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import26;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import27;

final List<Object> styles$I74UsaProviderFormas = const [];

class ViewI74UsaProviderFormas0 extends import0.ComponentView<import1.I74UsaProviderFormas> {
  late import2.I62Dep _I62Dep_0_6 = import2.I62Dep();
  late dynamic _I62Servico_0_7 = import2.criarI62Servico();
  late String _i62Rotulo_0_8 = import2.criarI62Rotulo(this._I62Dep_0_6);
  late dynamic _I62Outro_0_9 = import2.criarI62Outro(this._I62Dep_0_6);
  late import4.I63Servico _I63Servico_1_6 = import4.I63Servico();
  late List<String> _i64Nomes_2_6 = ['a', 'b'];
  late List<Object> _i64Validadores_2_7 = [import5.I64Validador(), this._I64ProviderMulti_2_5];
  late import6.I60Impl _I60Base_4_6 = import6.I60Impl();
  late import6.I60OutroImpl _I60Outro_4_7 = import6.I60OutroImpl();
  late import6.I60Terceiro _I60Terceiro_4_8 = import6.I60Terceiro();
  late final import7.ViewI62ProviderUseFactory0 _compView_0;
  late final import2.I62ProviderUseFactory _I62ProviderUseFactory_0_5;
  late final import8.ViewI63ProviderUseExisting0 _compView_1;
  late final import4.I63ProviderUseExisting _I63ProviderUseExisting_1_5;
  late final import9.ViewI64ProviderMulti0 _compView_2;
  late final import5.I64ProviderMulti _I64ProviderMulti_2_5;
  late final ViewContainer _appEl_3;
  late final NgIf _NgIf_3_9;
  late final import12.ViewI60ProviderUseClass0 _compView_4;
  late final import6.I60ProviderUseClass _I60ProviderUseClass_4_5;
  static import13.ComponentStyles? _componentStyles;
  ViewI74UsaProviderFormas0(import14.View parentView, int parentIndex) : super(parentView, parentIndex, import15.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import16.unsafeCast(import17.document.createElement('i74-usa-provider-formas'));
  }
  static String? get _debugComponentUrl {
    return (import16.isDevMode ? 'asset:corpus_ngdart/lib/src/i74_usa_provider_formas.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import7.ViewI62ProviderUseFactory0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._I62ProviderUseFactory_0_5 = import2.I62ProviderUseFactory();
    this._compView_0.create(this._I62ProviderUseFactory_0_5);
    this._compView_1 = import8.ViewI63ProviderUseExisting0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    parentRenderNode.append(_el_1);
    this._I63ProviderUseExisting_1_5 = import4.I63ProviderUseExisting();
    this._compView_1.create(this._I63ProviderUseExisting_1_5);
    this._compView_2 = import9.ViewI64ProviderMulti0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    parentRenderNode.append(_el_2);
    this._I64ProviderMulti_2_5 = import5.I64ProviderMulti();
    this._compView_2.create(this._I64ProviderMulti_2_5);
    final _anchor_3 = import18.appendAnchor(parentRenderNode);
    this._appEl_3 = ViewContainer(3, null, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_I74UsaProviderFormas1);
    this._NgIf_3_9 = NgIf(this._appEl_3, _TemplateRef_3_8);
    if (import20.isDevToolsEnabled) {
      import20.Inspector.instance.registerDirective(_anchor_3, this._NgIf_3_9);
    }
    this._compView_4 = import12.ViewI60ProviderUseClass0(this, 4);
    final _el_4 = this._compView_4.rootElement;
    parentRenderNode.append(_el_4);
    this._I60ProviderUseClass_4_5 = import6.I60ProviderUseClass();
    this._compView_4.create(this._I60ProviderUseClass_4_5);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import2.I62Dep)) {
        return this._I62Dep_0_6;
      }
      if (identical(token, import2.I62Servico)) {
        return this._I62Servico_0_7;
      }
      if (identical(token, const import21.OpaqueToken<String>('i62Rotulo'))) {
        return this._i62Rotulo_0_8;
      }
      if (identical(token, import2.I62Outro)) {
        return this._I62Outro_0_9;
      }
    }
    if ((1 == nodeIndex)) {
      if (identical(token, import4.I63Pai)) {
        return this._I63ProviderUseExisting_1_5;
      }
      if (((identical(token, import4.I63Servico) || identical(token, import4.I63Base)) || identical(token, import4.I63Leitor))) {
        return this._I63Servico_1_6;
      }
    }
    if ((2 == nodeIndex)) {
      if (identical(token, const import21.MultiToken<String>('i64Nomes'))) {
        return this._i64Nomes_2_6;
      }
      if (identical(token, const import21.MultiToken<Object>('i64Validadores'))) {
        return this._i64Validadores_2_7;
      }
    }
    if ((4 == nodeIndex)) {
      if (identical(token, import6.I60Base)) {
        return this._I60Base_4_6;
      }
      if (identical(token, import6.I60Outro)) {
        return this._I60Outro_4_7;
      }
      if (identical(token, import6.I60Terceiro)) {
        return this._I60Terceiro_4_8;
      }
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import20.isDevToolsEnabled) {
      import20.Inspector.instance.recordInput(this._NgIf_3_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_3_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/i74_usa_provider_formas.html:154:169 */;
    this._appEl_3.detectChangesInNestedViews();
    this._compView_0.detectChanges();
    this._compView_1.detectChanges();
    this._compView_2.detectChanges();
    this._compView_4.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_3.destroyNestedViews();
    this._compView_0.destroyInternalState();
    this._compView_1.destroyInternalState();
    this._compView_2.destroyInternalState();
    this._compView_4.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import13.ComponentStyles.unscoped(styles$I74UsaProviderFormas, _debugComponentUrl));
      if (import16.isDevMode) {
        import13.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I74UsaProviderFormasNgFactory = ComponentFactory<import1.I74UsaProviderFormas>('i74-usa-provider-formas', viewFactory_I74UsaProviderFormasHost0);
ComponentFactory<import1.I74UsaProviderFormas> get I74UsaProviderFormasNgFactory {
  return _I74UsaProviderFormasNgFactory;
}

ComponentFactory<import1.I74UsaProviderFormas> createI74UsaProviderFormasFactory() {
  return ComponentFactory('i74-usa-provider-formas', viewFactory_I74UsaProviderFormasHost0);
}

class _ViewI74UsaProviderFormas1 extends import23.EmbeddedView<import1.I74UsaProviderFormas> {
  late import24.I65A2 _I65A_1_6 = import24.I65A2();
  late import24.I65B _I65B_1_7 = import24.I65B();
  late import24.I65C _I65C_1_8 = import24.I65C();
  late import24.I65D _I65D_1_9 = import24.I65D();
  late final import25.ViewI65ProviderListas0 _compView_1;
  late final import24.I65ProviderListas _I65ProviderListas_1_5;
  _ViewI74UsaProviderFormas1(import26.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import17.document;
    final _el_0 = import16.unsafeCast(doc.createElement('div'));
    this._compView_1 = import25.ViewI65ProviderListas0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    _el_0.append(_el_1);
    this._I65ProviderListas_1_5 = import24.I65ProviderListas();
    this._compView_1.create(this._I65ProviderListas_1_5);
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((1 == nodeIndex)) {
      if (identical(token, import24.I65A)) {
        return this._I65A_1_6;
      }
      if (identical(token, import24.I65B)) {
        return this._I65B_1_7;
      }
      if (identical(token, import24.I65C)) {
        return this._I65C_1_8;
      }
      if (identical(token, import24.I65D)) {
        return this._I65D_1_9;
      }
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_1.destroyInternalState();
  }
}

import23.EmbeddedView<void> viewFactory_I74UsaProviderFormas1(import26.RenderView parentView, int parentIndex) {
  return _ViewI74UsaProviderFormas1(parentView, parentIndex);
}

final List<Object> styles$I74UsaProviderFormasHost = const [];

class _ViewI74UsaProviderFormasHost0 extends import27.HostView<import1.I74UsaProviderFormas> {
  @override
  void build() {
    this.componentView = ViewI74UsaProviderFormas0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I74UsaProviderFormas();
    this.initRootNode(_el_0);
  }
}

import27.HostView<import1.I74UsaProviderFormas> viewFactory_I74UsaProviderFormasHost0() {
  return _ViewI74UsaProviderFormasHost0();
}
