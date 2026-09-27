// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i71_provider_valores.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i71_provider_valores.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'dart:core';
import 'package:ngdart/src/meta/di_tokens.dart' as import11;

final List<Object> styles$I71ProviderValores = const [];

class ViewI71ProviderValores0 extends import0.ComponentView<import1.I71ProviderValores> {
  static import2.ComponentStyles? _componentStyles;
  ViewI71ProviderValores0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i71-provider-valores'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i71_provider_valores.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I71ProviderValores, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I71ProviderValoresNgFactory = ComponentFactory<import1.I71ProviderValores>('i71-provider-valores', viewFactory_I71ProviderValoresHost0);
ComponentFactory<import1.I71ProviderValores> get I71ProviderValoresNgFactory {
  return _I71ProviderValoresNgFactory;
}

ComponentFactory<import1.I71ProviderValores> createI71ProviderValoresFactory() {
  return ComponentFactory('i71-provider-valores', viewFactory_I71ProviderValoresHost0);
}

final List<Object> styles$I71ProviderValoresHost = const [];

class _ViewI71ProviderValoresHost0 extends import9.HostView<import1.I71ProviderValores> {
  late String _url_da_api_0_6 = '/api/\$x\'y';
  late import1.I71Config _I71Config_0_7 = const import1.I71Config('a', limite: 2, ativo: true);
  late List<String> _i71Itens_0_8 = ['um', this.injectorGet(import1.I71Externo, this.parentIndex)];
  late import1.I71Servico _I71Servico_0_9 = import1.I71Servico(this._url_da_api_0_6);
  @override
  void build() {
    this.componentView = ViewI71ProviderValores0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I71ProviderValores();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, const import11.OpaqueToken<String>('url-da-api'))) {
        return this._url_da_api_0_6;
      }
      if (identical(token, import1.I71Config)) {
        return this._I71Config_0_7;
      }
      if (identical(token, const import11.MultiToken<String>('i71Itens'))) {
        return this._i71Itens_0_8;
      }
      if ((identical(token, import1.I71Servico) || identical(token, import1.I71Leitor))) {
        return this._I71Servico_0_9;
      }
    }
    return notFoundResult;
  }
}

import9.HostView<import1.I71ProviderValores> viewFactory_I71ProviderValoresHost0() {
  return _ViewI71ProviderValoresHost0();
}
