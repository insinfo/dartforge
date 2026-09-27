// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i70_provider_ansioso_externo.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i70_provider_ansioso_externo.dart' as import1;
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

final List<Object> styles$I70ProviderAnsiosoExterno = const [];

class ViewI70ProviderAnsiosoExterno0 extends import0.ComponentView<import1.I70ProviderAnsiosoExterno> {
  static import2.ComponentStyles? _componentStyles;
  ViewI70ProviderAnsiosoExterno0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i70-provider-ansioso-externo'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i70_provider_ansioso_externo.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I70ProviderAnsiosoExterno, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I70ProviderAnsiosoExternoNgFactory = ComponentFactory<import1.I70ProviderAnsiosoExterno>('i70-provider-ansioso-externo', viewFactory_I70ProviderAnsiosoExternoHost0);
ComponentFactory<import1.I70ProviderAnsiosoExterno> get I70ProviderAnsiosoExternoNgFactory {
  return _I70ProviderAnsiosoExternoNgFactory;
}

ComponentFactory<import1.I70ProviderAnsiosoExterno> createI70ProviderAnsiosoExternoFactory() {
  return ComponentFactory('i70-provider-ansioso-externo', viewFactory_I70ProviderAnsiosoExternoHost0);
}

final List<Object> styles$I70ProviderAnsiosoExternoHost = const [];

class _ViewI70ProviderAnsiosoExternoHost0 extends import9.HostView<import1.I70ProviderAnsiosoExterno> {
  late final dynamic _I70Api_0_5;
  late final dynamic _I70Servico_0_6;
  late final import1.I70Impl _I70Base_0_7;
  @override
  void build() {
    this.componentView = ViewI70ProviderAnsiosoExterno0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this._I70Api_0_5 = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.I70Api, () {
            return import1.I70Api(this.injectorGet(import11.NgZone, this.parentIndex));
          })
        : import1.I70Api(this.injectorGet(import11.NgZone, this.parentIndex)));
    this._I70Servico_0_6 = import1.criarI70Servico(this._I70Api_0_5);
    this._I70Base_0_7 = import1.I70Impl();
    this.component = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.I70ProviderAnsiosoExterno, () {
            return import1.I70ProviderAnsiosoExterno(this._I70Api_0_5, this._I70Servico_0_6, this._I70Base_0_7, this.injectorGet(import11.NgZone, this.parentIndex));
          })
        : import1.I70ProviderAnsiosoExterno(this._I70Api_0_5, this._I70Servico_0_6, this._I70Base_0_7, this.injectorGet(import11.NgZone, this.parentIndex)));
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import1.I70Api)) {
        return this._I70Api_0_5;
      }
      if (identical(token, import1.I70Servico)) {
        return this._I70Servico_0_6;
      }
      if (identical(token, import1.I70Base)) {
        return this._I70Base_0_7;
      }
    }
    return notFoundResult;
  }
}

import9.HostView<import1.I70ProviderAnsiosoExterno> viewFactory_I70ProviderAnsiosoExternoHost0() {
  return _ViewI70ProviderAnsiosoExternoHost0();
}
