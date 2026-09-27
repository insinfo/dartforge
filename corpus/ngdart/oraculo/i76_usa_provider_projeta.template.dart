// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i76_usa_provider_projeta.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i76_usa_provider_projeta.dart' as import1;
import 'i60_provider_use_class.dart' as import2;
import 'i75_provider_projeta.template.dart' as import3;
import 'i75_provider_projeta.dart' as import4;
import 'i60_provider_use_class.template.dart' as import5;
import 'i75_leitor.dart' as import6;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import7;
import 'package:ngdart/src/core/linker/views/view.dart' as import8;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import9;
import 'package:ngdart/src/utilities.dart' as import10;
import 'dart:html' as import11;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import12;
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import15;

final List<Object> styles$I76UsaProviderProjeta = const [];

class ViewI76UsaProviderProjeta0 extends import0.ComponentView<import1.I76UsaProviderProjeta> {
  late import2.I60Impl _I60Base_1_6 = import2.I60Impl();
  late import2.I60OutroImpl _I60Outro_1_7 = import2.I60OutroImpl();
  late import2.I60Terceiro _I60Terceiro_1_8 = import2.I60Terceiro();
  late final import3.ViewI75ProviderProjeta0 _compView_0;
  late final import4.I75ProviderProjeta _I75ProviderProjeta_0_5;
  late final import4.I75Servico _I75Servico_0_6;
  late final import5.ViewI60ProviderUseClass0 _compView_1;
  late final import2.I60ProviderUseClass _I60ProviderUseClass_1_5;
  late final import6.I75Leitor _I75Leitor_2_5;
  static import7.ComponentStyles? _componentStyles;
  ViewI76UsaProviderProjeta0(import8.View parentView, int parentIndex) : super(parentView, parentIndex, import9.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import10.unsafeCast(import11.document.createElement('i76-usa-provider-projeta'));
  }
  static String? get _debugComponentUrl {
    return (import10.isDevMode ? 'asset:corpus_ngdart/lib/src/i76_usa_provider_projeta.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import3.ViewI75ProviderProjeta0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._I75ProviderProjeta_0_5 = import4.I75ProviderProjeta();
    this._I75Servico_0_6 = import4.I75Servico();
    this._compView_1 = import5.ViewI60ProviderUseClass0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    this._I60ProviderUseClass_1_5 = import2.I60ProviderUseClass();
    this._compView_1.create(this._I60ProviderUseClass_1_5);
    final doc = import11.document;
    final _el_2 = import10.unsafeCast(doc.createElement('p'));
    import12.setAttribute(_el_2, 'i75-leitor', '');
    this._I75Leitor_2_5 = import6.I75Leitor(this._I75Servico_0_6);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_2, this._I75Leitor_2_5);
    }
    final _text_3 = import12.appendText(_el_2, 't');
    this._compView_0.createAndProject(this._I75ProviderProjeta_0_5, [
      <Object>[_el_1, _el_2]
    ]);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((nodeIndex <= 3)) {
      if ((1 == nodeIndex)) {
        if (identical(token, import2.I60Base)) {
          return this._I60Base_1_6;
        }
        if (identical(token, import2.I60Outro)) {
          return this._I60Outro_1_7;
        }
        if (identical(token, import2.I60Terceiro)) {
          return this._I60Terceiro_1_8;
        }
      }
      if (identical(token, import4.I75Servico)) {
        return this._I75Servico_0_6;
      }
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._compView_1.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import7.ComponentStyles.unscoped(styles$I76UsaProviderProjeta, _debugComponentUrl));
      if (import10.isDevMode) {
        import7.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I76UsaProviderProjetaNgFactory = ComponentFactory<import1.I76UsaProviderProjeta>('i76-usa-provider-projeta', viewFactory_I76UsaProviderProjetaHost0);
ComponentFactory<import1.I76UsaProviderProjeta> get I76UsaProviderProjetaNgFactory {
  return _I76UsaProviderProjetaNgFactory;
}

ComponentFactory<import1.I76UsaProviderProjeta> createI76UsaProviderProjetaFactory() {
  return ComponentFactory('i76-usa-provider-projeta', viewFactory_I76UsaProviderProjetaHost0);
}

final List<Object> styles$I76UsaProviderProjetaHost = const [];

class _ViewI76UsaProviderProjetaHost0 extends import15.HostView<import1.I76UsaProviderProjeta> {
  @override
  void build() {
    this.componentView = ViewI76UsaProviderProjeta0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I76UsaProviderProjeta();
    this.initRootNode(_el_0);
  }
}

import15.HostView<import1.I76UsaProviderProjeta> viewFactory_I76UsaProviderProjetaHost0() {
  return _ViewI76UsaProviderProjetaHost0();
}
