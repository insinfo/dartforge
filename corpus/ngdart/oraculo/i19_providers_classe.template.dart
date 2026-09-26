// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i19_providers_classe.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i19_providers_classe.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$I19ProvidersClasse = const [];

class ViewI19ProvidersClasse0 extends import0.ComponentView<import1.I19ProvidersClasse> {
  static import2.ComponentStyles? _componentStyles;
  ViewI19ProvidersClasse0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i19-providers-classe'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i19_providers_classe.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_1 = import7.appendText(_el_0, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I19ProvidersClasse, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I19ProvidersClasseNgFactory = ComponentFactory<import1.I19ProvidersClasse>('i19-providers-classe', viewFactory_I19ProvidersClasseHost0);
ComponentFactory<import1.I19ProvidersClasse> get I19ProvidersClasseNgFactory {
  return _I19ProvidersClasseNgFactory;
}

ComponentFactory<import1.I19ProvidersClasse> createI19ProvidersClasseFactory() {
  return ComponentFactory('i19-providers-classe', viewFactory_I19ProvidersClasseHost0);
}

final List<Object> styles$I19ProvidersClasseHost = const [];

class _ViewI19ProvidersClasseHost0 extends import9.HostView<import1.I19ProvidersClasse> {
  late final import1.Servico _Servico_0_5;
  @override
  void build() {
    this.componentView = ViewI19ProvidersClasse0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this._Servico_0_5 = import1.Servico();
    this.component = import1.I19ProvidersClasse(this._Servico_0_5);
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.Servico) && (0 == nodeIndex))) {
      return this._Servico_0_5;
    }
    return notFoundResult;
  }
}

import9.HostView<import1.I19ProvidersClasse> viewFactory_I19ProvidersClasseHost0() {
  return _ViewI19ProvidersClasseHost0();
}
