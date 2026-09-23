// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'b18_providers_vazio.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'b18_providers_vazio.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$B18ProvidersVazio = const [];

class ViewB18ProvidersVazio0 extends import0.ComponentView<import1.B18ProvidersVazio> {
  static import2.ComponentStyles? _componentStyles;
  ViewB18ProvidersVazio0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('b18-providers-vazio'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/b18_providers_vazio.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$B18ProvidersVazio, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _B18ProvidersVazioNgFactory = ComponentFactory<import1.B18ProvidersVazio>('b18-providers-vazio', viewFactory_B18ProvidersVazioHost0);
ComponentFactory<import1.B18ProvidersVazio> get B18ProvidersVazioNgFactory {
  return _B18ProvidersVazioNgFactory;
}

ComponentFactory<import1.B18ProvidersVazio> createB18ProvidersVazioFactory() {
  return ComponentFactory('b18-providers-vazio', viewFactory_B18ProvidersVazioHost0);
}

final List<Object> styles$B18ProvidersVazioHost = const [];

class _ViewB18ProvidersVazioHost0 extends import9.HostView<import1.B18ProvidersVazio> {
  @override
  void build() {
    this.componentView = ViewB18ProvidersVazio0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.B18ProvidersVazio();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.B18ProvidersVazio> viewFactory_B18ProvidersVazioHost0() {
  return _ViewB18ProvidersVazioHost0();
}
