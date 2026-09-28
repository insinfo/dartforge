// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j44_varios_com_folha.dart';
import 'package:corpus_ngdart/src/j44_primeiro.css.shim.dart' as import0;
import 'package:ngdart/src/core/linker/views/component_view.dart' as import1;
import 'j44_varios_com_folha.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;
import 'package:corpus_ngdart/src/j44_terceiro.css.shim.dart' as import11;

final List<Object> styles$J44Primeiro = [import0.styles];

class ViewJ44Primeiro0 extends import1.ComponentView<import2.J44Primeiro> {
  static import3.ComponentStyles? _componentStyles;
  ViewJ44Primeiro0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j44-primeiro'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j44_varios_com_folha.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'b');
    this.addShimC(_el_0);
    final _text_1 = import8.appendText(_el_0, '1');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.scoped(styles$J44Primeiro, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J44PrimeiroNgFactory = ComponentFactory<import2.J44Primeiro>('j44-primeiro', viewFactory_J44PrimeiroHost0);
ComponentFactory<import2.J44Primeiro> get J44PrimeiroNgFactory {
  return _J44PrimeiroNgFactory;
}

ComponentFactory<import2.J44Primeiro> createJ44PrimeiroFactory() {
  return ComponentFactory('j44-primeiro', viewFactory_J44PrimeiroHost0);
}

final List<Object> styles$J44PrimeiroHost = const [];

class _ViewJ44PrimeiroHost0 extends import10.HostView<import2.J44Primeiro> {
  @override
  void build() {
    this.componentView = ViewJ44Primeiro0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import2.J44Primeiro();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import2.J44Primeiro> viewFactory_J44PrimeiroHost0() {
  return _ViewJ44PrimeiroHost0();
}

final List<Object> styles$J44Segundo = ['i._ngcontent-%ID%{color:red}'];

class ViewJ44Segundo0 extends import1.ComponentView<import2.J44Segundo> {
  late final ViewJ44Primeiro0 _compView_2;
  late final import2.J44Primeiro _J44Primeiro_2_5;
  static import3.ComponentStyles? _componentStyles;
  ViewJ44Segundo0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j44-segundo'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j44_varios_com_folha.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'i');
    this.addShimC(_el_0);
    final _text_1 = import8.appendText(_el_0, '2');
    this._compView_2 = ViewJ44Primeiro0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    parentRenderNode.append(_el_2);
    this.addShimC(_el_2);
    this._J44Primeiro_2_5 = import2.J44Primeiro();
    this._compView_2.create(this._J44Primeiro_2_5);
  }

  @override
  void detectChangesInternal() {
    this._compView_2.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_2.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.scoped(styles$J44Segundo, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J44SegundoNgFactory = ComponentFactory<import2.J44Segundo>('j44-segundo', viewFactory_J44SegundoHost0);
ComponentFactory<import2.J44Segundo> get J44SegundoNgFactory {
  return _J44SegundoNgFactory;
}

ComponentFactory<import2.J44Segundo> createJ44SegundoFactory() {
  return ComponentFactory('j44-segundo', viewFactory_J44SegundoHost0);
}

final List<Object> styles$J44SegundoHost = const [];

class _ViewJ44SegundoHost0 extends import10.HostView<import2.J44Segundo> {
  @override
  void build() {
    this.componentView = ViewJ44Segundo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import2.J44Segundo();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import2.J44Segundo> viewFactory_J44SegundoHost0() {
  return _ViewJ44SegundoHost0();
}

final List<Object> styles$J44Terceiro = [import11.styles, import0.styles];

class ViewJ44Terceiro0 extends import1.ComponentView<import2.J44Terceiro> {
  late final ViewJ44Segundo0 _compView_2;
  late final import2.J44Segundo _J44Segundo_2_5;
  static import3.ComponentStyles? _componentStyles;
  ViewJ44Terceiro0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j44-terceiro'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j44_varios_com_folha.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'u');
    this.addShimC(_el_0);
    final _text_1 = import8.appendText(_el_0, '3');
    this._compView_2 = ViewJ44Segundo0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    parentRenderNode.append(_el_2);
    this.addShimC(_el_2);
    this._J44Segundo_2_5 = import2.J44Segundo();
    this._compView_2.create(this._J44Segundo_2_5);
  }

  @override
  void detectChangesInternal() {
    this._compView_2.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_2.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.scoped(styles$J44Terceiro, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J44TerceiroNgFactory = ComponentFactory<import2.J44Terceiro>('j44-terceiro', viewFactory_J44TerceiroHost0);
ComponentFactory<import2.J44Terceiro> get J44TerceiroNgFactory {
  return _J44TerceiroNgFactory;
}

ComponentFactory<import2.J44Terceiro> createJ44TerceiroFactory() {
  return ComponentFactory('j44-terceiro', viewFactory_J44TerceiroHost0);
}

final List<Object> styles$J44TerceiroHost = const [];

class _ViewJ44TerceiroHost0 extends import10.HostView<import2.J44Terceiro> {
  @override
  void build() {
    this.componentView = ViewJ44Terceiro0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import2.J44Terceiro();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import2.J44Terceiro> viewFactory_J44TerceiroHost0() {
  return _ViewJ44TerceiroHost0();
}
