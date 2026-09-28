// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j79_host_e_self.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j79_host_e_self.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/di/errors.dart' as import10;

final List<Object> styles$J79Linha = const [];

class ViewJ79Linha0 extends import0.ComponentView<import1.J79Linha> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ79Linha0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j79-linha'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j79_host_e_self.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this.project(parentRenderNode, 0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J79Linha, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J79LinhaNgFactory = ComponentFactory<import1.J79Linha>('j79-linha', viewFactory_J79LinhaHost0);
ComponentFactory<import1.J79Linha> get J79LinhaNgFactory {
  return _J79LinhaNgFactory;
}

ComponentFactory<import1.J79Linha> createJ79LinhaFactory() {
  return ComponentFactory('j79-linha', viewFactory_J79LinhaHost0);
}

final List<Object> styles$J79LinhaHost = const [];

class _ViewJ79LinhaHost0 extends import8.HostView<import1.J79Linha> {
  @override
  void build() {
    this.componentView = ViewJ79Linha0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J79Linha();
    this.initRootNode(_el_0);
  }
}

import8.HostView<import1.J79Linha> viewFactory_J79LinhaHost0() {
  return _ViewJ79LinhaHost0();
}

final List<Object> styles$J79Item = const [];

class ViewJ79Item0 extends import0.ComponentView<import1.J79Item> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ79Item0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j79-item'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j79_host_e_self.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import9.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J79Item, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J79ItemNgFactory = ComponentFactory<import1.J79Item>('j79-item', viewFactory_J79ItemHost0);
ComponentFactory<import1.J79Item> get J79ItemNgFactory {
  return _J79ItemNgFactory;
}

ComponentFactory<import1.J79Item> createJ79ItemFactory() {
  return ComponentFactory('j79-item', viewFactory_J79ItemHost0);
}

final List<Object> styles$J79ItemHost = const [];

class _ViewJ79ItemHost0 extends import8.HostView<import1.J79Item> {
  @override
  void build() {
    this.componentView = ViewJ79Item0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.J79Item, () {
            return import1.J79Item(this.injectorGetOptional(import1.J79Linha, this.parentIndex), null, this.injectorGetOptional(import1.J79Servico, this.parentIndex), this.injectorGetOptional(import1.J79Servico, this.parentIndex));
          })
        : import1.J79Item(this.injectorGetOptional(import1.J79Linha, this.parentIndex), null, this.injectorGetOptional(import1.J79Servico, this.parentIndex), this.injectorGetOptional(import1.J79Servico, this.parentIndex)));
    this.initRootNode(_el_0);
  }
}

import8.HostView<import1.J79Item> viewFactory_J79ItemHost0() {
  return _ViewJ79ItemHost0();
}

final List<Object> styles$J79HostESelf = const [];

class ViewJ79HostESelf0 extends import0.ComponentView<import1.J79HostESelf> {
  late final ViewJ79Linha0 _compView_0;
  late final import1.J79Linha _J79Linha_0_5;
  late final ViewJ79Item0 _compView_1;
  late final import1.J79Item _J79Item_1_5;
  late final ViewJ79Item0 _compView_2;
  late final import1.J79Item _J79Item_2_5;
  static import2.ComponentStyles? _componentStyles;
  ViewJ79HostESelf0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j79-host-e-self'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j79_host_e_self.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ79Linha0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J79Linha_0_5 = import1.J79Linha();
    this._compView_1 = ViewJ79Item0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    this._J79Item_1_5 = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.J79Item, () {
            return import1.J79Item(this._J79Linha_0_5, null, (this.parentView!).injectorGetOptional(import1.J79Servico, this.parentIndex), null);
          })
        : import1.J79Item(this._J79Linha_0_5, null, (this.parentView!).injectorGetOptional(import1.J79Servico, this.parentIndex), null));
    this._compView_1.create(this._J79Item_1_5);
    this._compView_0.createAndProject(this._J79Linha_0_5, [
      <Object>[_el_1]
    ]);
    this._compView_2 = ViewJ79Item0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    parentRenderNode.append(_el_2);
    this._J79Item_2_5 = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.J79Item, () {
            return import1.J79Item(null, null, (this.parentView!).injectorGetOptional(import1.J79Servico, this.parentIndex), null);
          })
        : import1.J79Item(null, null, (this.parentView!).injectorGetOptional(import1.J79Servico, this.parentIndex), null));
    this._compView_2.create(this._J79Item_2_5);
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
    this._compView_1.detectChanges();
    this._compView_2.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._compView_1.destroyInternalState();
    this._compView_2.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J79HostESelf, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J79HostESelfNgFactory = ComponentFactory<import1.J79HostESelf>('j79-host-e-self', viewFactory_J79HostESelfHost0);
ComponentFactory<import1.J79HostESelf> get J79HostESelfNgFactory {
  return _J79HostESelfNgFactory;
}

ComponentFactory<import1.J79HostESelf> createJ79HostESelfFactory() {
  return ComponentFactory('j79-host-e-self', viewFactory_J79HostESelfHost0);
}

final List<Object> styles$J79HostESelfHost = const [];

class _ViewJ79HostESelfHost0 extends import8.HostView<import1.J79HostESelf> {
  @override
  void build() {
    this.componentView = ViewJ79HostESelf0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J79HostESelf();
    this.initRootNode(_el_0);
  }
}

import8.HostView<import1.J79HostESelf> viewFactory_J79HostESelfHost0() {
  return _ViewJ79HostESelfHost0();
}
