// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i65_provider_listas.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i65_provider_listas.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;

final List<Object> styles$I65ProviderListas = const [];

class ViewI65ProviderListas0 extends import0.ComponentView<import1.I65ProviderListas> {
  static import2.ComponentStyles? _componentStyles;
  ViewI65ProviderListas0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i65-provider-listas'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i65_provider_listas.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I65ProviderListas, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I65ProviderListasNgFactory = ComponentFactory<import1.I65ProviderListas>('i65-provider-listas', viewFactory_I65ProviderListasHost0);
ComponentFactory<import1.I65ProviderListas> get I65ProviderListasNgFactory {
  return _I65ProviderListasNgFactory;
}

ComponentFactory<import1.I65ProviderListas> createI65ProviderListasFactory() {
  return ComponentFactory('i65-provider-listas', viewFactory_I65ProviderListasHost0);
}

final List<Object> styles$I65ProviderListasHost = const [];

class _ViewI65ProviderListasHost0 extends import9.HostView<import1.I65ProviderListas> {
  late import1.I65A2 _I65A_0_6 = import1.I65A2();
  late import1.I65B _I65B_0_7 = import1.I65B();
  late import1.I65C _I65C_0_8 = import1.I65C();
  late import1.I65D _I65D_0_9 = import1.I65D();
  @override
  void build() {
    this.componentView = ViewI65ProviderListas0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I65ProviderListas();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import1.I65A)) {
        return this._I65A_0_6;
      }
      if (identical(token, import1.I65B)) {
        return this._I65B_0_7;
      }
      if (identical(token, import1.I65C)) {
        return this._I65C_0_8;
      }
      if (identical(token, import1.I65D)) {
        return this._I65D_0_9;
      }
    }
    return notFoundResult;
  }
}

import9.HostView<import1.I65ProviderListas> viewFactory_I65ProviderListasHost0() {
  return _ViewI65ProviderListasHost0();
}
