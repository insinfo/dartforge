// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b02_providers.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'b02_providers.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$B02Providers = const [];

class ViewB02Providers0 extends import0.ComponentView<import1.B02Providers> {
  static import2.ComponentStyles? _componentStyles;
  ViewB02Providers0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('b02-providers'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/b02_providers.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    final _text_1 = import7.appendText(_el_0, 'oi');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$B02Providers, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B02ProvidersNgFactory = ComponentFactory<import1.B02Providers>('b02-providers', viewFactory_B02ProvidersHost0);
ComponentFactory<import1.B02Providers> get B02ProvidersNgFactory {
  return _B02ProvidersNgFactory;
}

ComponentFactory<import1.B02Providers> createB02ProvidersFactory() {
  return ComponentFactory('b02-providers', viewFactory_B02ProvidersHost0);
}

final List<Object> styles$B02ProvidersHost = const [];

class _ViewB02ProvidersHost0 extends import9.HostView<import1.B02Providers> {
  late import1.Servico _Servico_0_6 = import1.Servico();
  @override
  void build() {
    this.componentView = ViewB02Providers0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.B02Providers();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.Servico) && (0 == nodeIndex))) {
      return this._Servico_0_6;
    }
    return notFoundResult;
  }
}

import9.HostView<import1.B02Providers> viewFactory_B02ProvidersHost0() {
  return _ViewB02ProvidersHost0();
}
