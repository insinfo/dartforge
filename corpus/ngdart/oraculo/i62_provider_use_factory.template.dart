// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i62_provider_use_factory.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i62_provider_use_factory.dart' as import1;
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

final List<Object> styles$I62ProviderUseFactory = const [];

class ViewI62ProviderUseFactory0 extends import0.ComponentView<import1.I62ProviderUseFactory> {
  static import2.ComponentStyles? _componentStyles;
  ViewI62ProviderUseFactory0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i62-provider-use-factory'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i62_provider_use_factory.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I62ProviderUseFactory, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I62ProviderUseFactoryNgFactory = ComponentFactory<import1.I62ProviderUseFactory>('i62-provider-use-factory', viewFactory_I62ProviderUseFactoryHost0);
ComponentFactory<import1.I62ProviderUseFactory> get I62ProviderUseFactoryNgFactory {
  return _I62ProviderUseFactoryNgFactory;
}

ComponentFactory<import1.I62ProviderUseFactory> createI62ProviderUseFactoryFactory() {
  return ComponentFactory('i62-provider-use-factory', viewFactory_I62ProviderUseFactoryHost0);
}

final List<Object> styles$I62ProviderUseFactoryHost = const [];

class _ViewI62ProviderUseFactoryHost0 extends import9.HostView<import1.I62ProviderUseFactory> {
  late import1.I62Dep _I62Dep_0_6 = import1.I62Dep();
  late dynamic _I62Servico_0_7 = import1.criarI62Servico();
  late String _i62Rotulo_0_8 = import1.criarI62Rotulo(this._I62Dep_0_6);
  late dynamic _I62Outro_0_9 = import1.criarI62Outro(this._I62Dep_0_6);
  @override
  void build() {
    this.componentView = ViewI62ProviderUseFactory0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I62ProviderUseFactory();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import1.I62Dep)) {
        return this._I62Dep_0_6;
      }
      if (identical(token, import1.I62Servico)) {
        return this._I62Servico_0_7;
      }
      if (identical(token, const import11.OpaqueToken<String>('i62Rotulo'))) {
        return this._i62Rotulo_0_8;
      }
      if (identical(token, import1.I62Outro)) {
        return this._I62Outro_0_9;
      }
    }
    return notFoundResult;
  }
}

import9.HostView<import1.I62ProviderUseFactory> viewFactory_I62ProviderUseFactoryHost0() {
  return _ViewI62ProviderUseFactoryHost0();
}
