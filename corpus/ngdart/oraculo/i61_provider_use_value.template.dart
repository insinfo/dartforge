// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i61_provider_use_value.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i61_provider_use_value.dart' as import1;
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

final List<Object> styles$I61ProviderUseValue = const [];

class ViewI61ProviderUseValue0 extends import0.ComponentView<import1.I61ProviderUseValue> {
  static import2.ComponentStyles? _componentStyles;
  ViewI61ProviderUseValue0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i61-provider-use-value'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i61_provider_use_value.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I61ProviderUseValue, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I61ProviderUseValueNgFactory = ComponentFactory<import1.I61ProviderUseValue>('i61-provider-use-value', viewFactory_I61ProviderUseValueHost0);
ComponentFactory<import1.I61ProviderUseValue> get I61ProviderUseValueNgFactory {
  return _I61ProviderUseValueNgFactory;
}

ComponentFactory<import1.I61ProviderUseValue> createI61ProviderUseValueFactory() {
  return ComponentFactory('i61-provider-use-value', viewFactory_I61ProviderUseValueHost0);
}

final List<Object> styles$I61ProviderUseValueHost = const [];

class _ViewI61ProviderUseValueHost0 extends import9.HostView<import1.I61ProviderUseValue> {
  late String _i61Nome_0_6 = 'sonda';
  late int _i61Limite_0_7 = 10;
  late bool _i61Ativo_0_8 = true;
  late import1.I61Config _I61Config_0_9 = const import1.I61Config('/api');
  @override
  void build() {
    this.componentView = ViewI61ProviderUseValue0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I61ProviderUseValue();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, const import11.OpaqueToken<String>('i61Nome'))) {
        return this._i61Nome_0_6;
      }
      if (identical(token, const import11.OpaqueToken<int>('i61Limite'))) {
        return this._i61Limite_0_7;
      }
      if (identical(token, const import11.OpaqueToken<bool>('i61Ativo'))) {
        return this._i61Ativo_0_8;
      }
      if (identical(token, import1.I61Config)) {
        return this._I61Config_0_9;
      }
    }
    return notFoundResult;
  }
}

import9.HostView<import1.I61ProviderUseValue> viewFactory_I61ProviderUseValueHost0() {
  return _ViewI61ProviderUseValueHost0();
}
