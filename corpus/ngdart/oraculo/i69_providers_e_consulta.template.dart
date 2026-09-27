// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i69_providers_e_consulta.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i69_providers_e_consulta.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import8;
import 'package:ngdart/src/runtime/check_binding.dart' as import9;

final List<Object> styles$I69ProvidersEConsulta = const [];

class ViewI69ProvidersEConsulta0 extends import0.ComponentView<import1.I69ProvidersEConsulta> {
  static import2.ComponentStyles? _componentStyles;
  ViewI69ProvidersEConsulta0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i69-providers-e-consulta'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i69_providers_e_consulta.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this.project(parentRenderNode, 0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I69ProvidersEConsulta, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I69ProvidersEConsultaNgFactory = ComponentFactory<import1.I69ProvidersEConsulta>('i69-providers-e-consulta', viewFactory_I69ProvidersEConsultaHost0);
ComponentFactory<import1.I69ProvidersEConsulta> get I69ProvidersEConsultaNgFactory {
  return _I69ProvidersEConsultaNgFactory;
}

ComponentFactory<import1.I69ProvidersEConsulta> createI69ProvidersEConsultaFactory() {
  return ComponentFactory('i69-providers-e-consulta', viewFactory_I69ProvidersEConsultaHost0);
}

final List<Object> styles$I69ProvidersEConsultaHost = const [];

class _ViewI69ProvidersEConsultaHost0 extends import8.HostView<import1.I69ProvidersEConsulta> {
  late import1.I69Outro _I69Outro_0_7 = import1.I69Outro();
  late final import1.I69Servico _I69Servico_0_5;
  @override
  void build() {
    this.componentView = ViewI69ProvidersEConsulta0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this._I69Servico_0_5 = import1.I69Servico();
    this.component = import1.I69ProvidersEConsulta(this._I69Servico_0_5);
    this.component.marcas = [];
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import1.I69Servico)) {
        return this._I69Servico_0_5;
      }
      if (identical(token, import1.I69Outro)) {
        return this._I69Outro_0_7;
      }
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if (((!import9.debugThrowIfChanged) && firstCheck)) {
      this.component.ngOnInit();
    }
    this.componentView.detectChanges();
  }
}

import8.HostView<import1.I69ProvidersEConsulta> viewFactory_I69ProvidersEConsultaHost0() {
  return _ViewI69ProvidersEConsultaHost0();
}
