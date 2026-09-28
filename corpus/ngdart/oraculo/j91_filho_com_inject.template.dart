// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j91_filho_com_inject.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j91_filho_com_inject.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/interpolate.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;
import 'package:ngdart/src/di/errors.dart' as import11;
import 'package:ngdart/src/meta/di_tokens.dart' as import12;
import 'dart:core';

final List<Object> styles$J91Filho = const [];

class ViewJ91Filho0 extends import0.ComponentView<import1.J91Filho> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ91Filho0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j91-filho'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j91_filho_com_inject.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
    final _text_1 = import7.appendText(_el_0, import8.interpolateString0(_ctx.tipo));
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J91Filho, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J91FilhoNgFactory = ComponentFactory<import1.J91Filho>('j91-filho', viewFactory_J91FilhoHost0);
ComponentFactory<import1.J91Filho> get J91FilhoNgFactory {
  return _J91FilhoNgFactory;
}

ComponentFactory<import1.J91Filho> createJ91FilhoFactory() {
  return ComponentFactory('j91-filho', viewFactory_J91FilhoHost0);
}

final List<Object> styles$J91FilhoHost = const [];

class _ViewJ91FilhoHost0 extends import10.HostView<import1.J91Filho> {
  @override
  void build() {
    this.componentView = ViewJ91Filho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = (import5.isDevMode
        ? import11.debugInjectorWrap(import1.J91Filho, () {
            return import1.J91Filho(null, null, this.injectorGet(const import12.OpaqueToken<String>('j91Nome'), this.parentIndex), this.injectorGetOptional(const import12.MultiToken<String>('j91Lista'), this.parentIndex), this.injectorGet(import1.J91Servico, this.parentIndex));
          })
        : import1.J91Filho(null, null, this.injectorGet(const import12.OpaqueToken<String>('j91Nome'), this.parentIndex), this.injectorGetOptional(const import12.MultiToken<String>('j91Lista'), this.parentIndex), this.injectorGet(import1.J91Servico, this.parentIndex)));
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.J91Filho> viewFactory_J91FilhoHost0() {
  return _ViewJ91FilhoHost0();
}

final List<Object> styles$J91FilhoComInject = const [];

class ViewJ91FilhoComInject0 extends import0.ComponentView<import1.J91FilhoComInject> {
  late final ViewJ91Filho0 _compView_0;
  late final import1.J91Filho _J91Filho_0_5;
  late final ViewJ91Filho0 _compView_1;
  late final import1.J91Filho _J91Filho_1_5;
  static import2.ComponentStyles? _componentStyles;
  ViewJ91FilhoComInject0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j91-filho-com-inject'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j91_filho_com_inject.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ91Filho0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    import7.setAttribute(_el_0, 'tipo', 'grande');
    this._J91Filho_0_5 = (import5.isDevMode
        ? import11.debugInjectorWrap(import1.J91Filho, () {
            return import1.J91Filho('grande', null, (this.parentView!).injectorGet(const import12.OpaqueToken<String>('j91Nome'), this.parentIndex), (this.parentView!).injectorGetOptional(const import12.MultiToken<String>('j91Lista'), this.parentIndex), (this.parentView!).injectorGet(import1.J91Servico, this.parentIndex));
          })
        : import1.J91Filho('grande', null, (this.parentView!).injectorGet(const import12.OpaqueToken<String>('j91Nome'), this.parentIndex), (this.parentView!).injectorGetOptional(const import12.MultiToken<String>('j91Lista'), this.parentIndex), (this.parentView!).injectorGet(import1.J91Servico, this.parentIndex)));
    this._compView_0.create(this._J91Filho_0_5);
    this._compView_1 = ViewJ91Filho0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    parentRenderNode.append(_el_1);
    this._J91Filho_1_5 = (import5.isDevMode
        ? import11.debugInjectorWrap(import1.J91Filho, () {
            return import1.J91Filho(null, null, (this.parentView!).injectorGet(const import12.OpaqueToken<String>('j91Nome'), this.parentIndex), (this.parentView!).injectorGetOptional(const import12.MultiToken<String>('j91Lista'), this.parentIndex), (this.parentView!).injectorGet(import1.J91Servico, this.parentIndex));
          })
        : import1.J91Filho(null, null, (this.parentView!).injectorGet(const import12.OpaqueToken<String>('j91Nome'), this.parentIndex), (this.parentView!).injectorGetOptional(const import12.MultiToken<String>('j91Lista'), this.parentIndex), (this.parentView!).injectorGet(import1.J91Servico, this.parentIndex)));
    this._compView_1.create(this._J91Filho_1_5);
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._compView_1.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J91FilhoComInject, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J91FilhoComInjectNgFactory = ComponentFactory<import1.J91FilhoComInject>('j91-filho-com-inject', viewFactory_J91FilhoComInjectHost0);
ComponentFactory<import1.J91FilhoComInject> get J91FilhoComInjectNgFactory {
  return _J91FilhoComInjectNgFactory;
}

ComponentFactory<import1.J91FilhoComInject> createJ91FilhoComInjectFactory() {
  return ComponentFactory('j91-filho-com-inject', viewFactory_J91FilhoComInjectHost0);
}

final List<Object> styles$J91FilhoComInjectHost = const [];

class _ViewJ91FilhoComInjectHost0 extends import10.HostView<import1.J91FilhoComInject> {
  late import1.J91Servico _J91Servico_0_6 = import1.J91Servico();
  late String _j91Nome_0_7 = 'x';
  @override
  void build() {
    this.componentView = ViewJ91FilhoComInject0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J91FilhoComInject();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import1.J91Servico)) {
        return this._J91Servico_0_6;
      }
      if (identical(token, const import12.OpaqueToken<String>('j91Nome'))) {
        return this._j91Nome_0_7;
      }
    }
    return notFoundResult;
  }
}

import10.HostView<import1.J91FilhoComInject> viewFactory_J91FilhoComInjectHost0() {
  return _ViewJ91FilhoComInjectHost0();
}
