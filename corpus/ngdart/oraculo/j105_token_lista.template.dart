// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j105_token_lista.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j105_token_lista.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/di/errors.dart' as import10;
import 'package:ngdart/src/meta/di_tokens.dart' as import11;
import 'dart:core';
import 'package:corpus_ngdart/src/j105_posicao.dart' as import13;

final List<Object> styles$J105Popup = const [];

class ViewJ105Popup0 extends import0.ComponentView<import1.J105Popup> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ105Popup0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j105-popup'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j105_token_lista.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
    final _text_1 = import7.appendText(_el_0, 'x');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J105Popup, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J105PopupNgFactory = ComponentFactory<import1.J105Popup>('j105-popup', viewFactory_J105PopupHost0);
ComponentFactory<import1.J105Popup> get J105PopupNgFactory {
  return _J105PopupNgFactory;
}

ComponentFactory<import1.J105Popup> createJ105PopupFactory() {
  return ComponentFactory('j105-popup', viewFactory_J105PopupHost0);
}

final List<Object> styles$J105PopupHost = const [];

class _ViewJ105PopupHost0 extends import9.HostView<import1.J105Popup> {
  @override
  void build() {
    this.componentView = ViewJ105Popup0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.J105Popup, () {
            return import1.J105Popup(this.injectorGet(const import11.OpaqueToken<List<import13.J105Posicao>>('j105Posicoes'), this.parentIndex), this.injectorGetOptional(const import11.OpaqueToken<Map<String, bool>>('j105Mapa'), this.parentIndex));
          })
        : import1.J105Popup(this.injectorGet(const import11.OpaqueToken<List<import13.J105Posicao>>('j105Posicoes'), this.parentIndex), this.injectorGetOptional(const import11.OpaqueToken<Map<String, bool>>('j105Mapa'), this.parentIndex)));
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J105Popup> viewFactory_J105PopupHost0() {
  return _ViewJ105PopupHost0();
}

final List<Object> styles$J105Usa = const [];

class ViewJ105Usa0 extends import0.ComponentView<import1.J105Usa> {
  late final ViewJ105Popup0 _compView_0;
  late final import1.J105Popup _J105Popup_0_5;
  static import2.ComponentStyles? _componentStyles;
  ViewJ105Usa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j105-usa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j105_token_lista.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ105Popup0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J105Popup_0_5 = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.J105Popup, () {
            return import1.J105Popup((this.parentView!).injectorGet(const import11.OpaqueToken<List<import13.J105Posicao>>('j105Posicoes'), this.parentIndex), (this.parentView!).injectorGetOptional(const import11.OpaqueToken<Map<String, bool>>('j105Mapa'), this.parentIndex));
          })
        : import1.J105Popup((this.parentView!).injectorGet(const import11.OpaqueToken<List<import13.J105Posicao>>('j105Posicoes'), this.parentIndex), (this.parentView!).injectorGetOptional(const import11.OpaqueToken<Map<String, bool>>('j105Mapa'), this.parentIndex)));
    this._compView_0.create(this._J105Popup_0_5);
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J105Usa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J105UsaNgFactory = ComponentFactory<import1.J105Usa>('j105-usa', viewFactory_J105UsaHost0);
ComponentFactory<import1.J105Usa> get J105UsaNgFactory {
  return _J105UsaNgFactory;
}

ComponentFactory<import1.J105Usa> createJ105UsaFactory() {
  return ComponentFactory('j105-usa', viewFactory_J105UsaHost0);
}

final List<Object> styles$J105UsaHost = const [];

class _ViewJ105UsaHost0 extends import9.HostView<import1.J105Usa> {
  @override
  void build() {
    this.componentView = ViewJ105Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J105Usa();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J105Usa> viewFactory_J105UsaHost0() {
  return _ViewJ105UsaHost0();
}
