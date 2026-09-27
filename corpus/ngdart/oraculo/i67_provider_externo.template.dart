// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i67_provider_externo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i67_provider_externo.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/di/errors.dart' as import10;
import 'package:ngdart/src/core/zone/ng_zone.dart' as import11;

final List<Object> styles$I67ProviderExterno = const [];

class ViewI67ProviderExterno0 extends import0.ComponentView<import1.I67ProviderExterno> {
  static import2.ComponentStyles? _componentStyles;
  ViewI67ProviderExterno0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i67-provider-externo'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i67_provider_externo.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I67ProviderExterno, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I67ProviderExternoNgFactory = ComponentFactory<import1.I67ProviderExterno>('i67-provider-externo', viewFactory_I67ProviderExternoHost0);
ComponentFactory<import1.I67ProviderExterno> get I67ProviderExternoNgFactory {
  return _I67ProviderExternoNgFactory;
}

ComponentFactory<import1.I67ProviderExterno> createI67ProviderExternoFactory() {
  return ComponentFactory('i67-provider-externo', viewFactory_I67ProviderExternoHost0);
}

final List<Object> styles$I67ProviderExternoHost = const [];

class _ViewI67ProviderExternoHost0 extends import9.HostView<import1.I67ProviderExterno> {
  late dynamic _I67Local_0_6 = (import5.isDevMode
      ? import10.debugInjectorWrap(import1.I67Local, () {
          return import1.I67Local(this.injectorGet(import11.NgZone, this.parentIndex));
        })
      : import1.I67Local(this.injectorGet(import11.NgZone, this.parentIndex)));
  late dynamic _I67Apelido_0_7 = this.injectorGet(import1.I67Externo, this.parentIndex);
  @override
  void build() {
    this.componentView = ViewI67ProviderExterno0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I67ProviderExterno();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import1.I67Local)) {
        return this._I67Local_0_6;
      }
      if (identical(token, import1.I67Apelido)) {
        return this._I67Apelido_0_7;
      }
    }
    return notFoundResult;
  }
}

import9.HostView<import1.I67ProviderExterno> viewFactory_I67ProviderExternoHost0() {
  return _ViewI67ProviderExternoHost0();
}
