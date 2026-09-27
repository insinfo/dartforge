// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i75_provider_projeta.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i75_provider_projeta.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import8;

final List<Object> styles$I75ProviderProjeta = const [];

class ViewI75ProviderProjeta0 extends import0.ComponentView<import1.I75ProviderProjeta> {
  static import2.ComponentStyles? _componentStyles;
  ViewI75ProviderProjeta0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i75-provider-projeta'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i75_provider_projeta.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I75ProviderProjeta, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I75ProviderProjetaNgFactory = ComponentFactory<import1.I75ProviderProjeta>('i75-provider-projeta', viewFactory_I75ProviderProjetaHost0);
ComponentFactory<import1.I75ProviderProjeta> get I75ProviderProjetaNgFactory {
  return _I75ProviderProjetaNgFactory;
}

ComponentFactory<import1.I75ProviderProjeta> createI75ProviderProjetaFactory() {
  return ComponentFactory('i75-provider-projeta', viewFactory_I75ProviderProjetaHost0);
}

final List<Object> styles$I75ProviderProjetaHost = const [];

class _ViewI75ProviderProjetaHost0 extends import8.HostView<import1.I75ProviderProjeta> {
  late import1.I75Servico _I75Servico_0_6 = import1.I75Servico();
  @override
  void build() {
    this.componentView = ViewI75ProviderProjeta0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I75ProviderProjeta();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.I75Servico) && (0 == nodeIndex))) {
      return this._I75Servico_0_6;
    }
    return notFoundResult;
  }
}

import8.HostView<import1.I75ProviderProjeta> viewFactory_I75ProviderProjetaHost0() {
  return _ViewI75ProviderProjetaHost0();
}
