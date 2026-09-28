// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j126_tipos_de_diretiva.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j126_tipos_de_diretiva.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/interpolate.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;
import 'dart:core';
import 'package:ngdart/src/runtime/dom_helpers.dart' as import12;
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/src/runtime/check_binding.dart' as import14;

final List<Object> styles$J126Item = const [];

class ViewJ126Item0<T> extends import0.ComponentView<import1.J126Item<T>> {
  final import2.TextBinding _textBinding_0 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewJ126Item0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j126-item'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j126_tipos_de_diretiva.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    parentRenderNode.append(this._textBinding_0.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_0.updateText(import8.interpolate0(_ctx.valor)) /* REF:asset:corpus_ngdart/lib/src/j126_tipos_de_diretiva.dart:371:380 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J126Item, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J126ItemNgFactory = ComponentFactory<import1.J126Item>('j126-item', viewFactory_J126ItemHost0);
ComponentFactory<import1.J126Item> get J126ItemNgFactory {
  return _J126ItemNgFactory;
}

ComponentFactory<import1.J126Item<T>> createJ126ItemFactory<T>() {
  return ComponentFactory('j126-item', viewFactory_J126ItemHost0);
}

final List<Object> styles$J126ItemHost = const [];

class _ViewJ126ItemHost0<T> extends import10.HostView<import1.J126Item<T>> {
  @override
  void build() {
    this.componentView = ViewJ126Item0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J126Item();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.J126Item<T>> viewFactory_J126ItemHost0<T>() {
  return _ViewJ126ItemHost0();
}

final List<Object> styles$J126Lista = const [];

class ViewJ126Lista0<T> extends import0.ComponentView<import1.J126Lista<T>> {
  late final ViewJ126Item0<T> _compView_0;
  late final import1.J126Item<T> _J126Item_0_5;
  late final import1.J126Marca<String> _J126Marca_1_5;
  late final import1.J126Marca _J126Marca_2_5;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  static import3.ComponentStyles? _componentStyles;
  ViewJ126Lista0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j126-lista'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j126_tipos_de_diretiva.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ126Item0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J126Item_0_5 = import1.J126Item();
    this._compView_0.create(this._J126Item_0_5);
    final doc = import7.document;
    final _el_1 = import12.appendDiv(doc, parentRenderNode);
    import12.setAttribute(_el_1, 'j126Marca', '');
    this._J126Marca_1_5 = import1.J126Marca();
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_1, this._J126Marca_1_5);
    }
    final _el_2 = import12.appendDiv(doc, parentRenderNode);
    import12.setAttribute(_el_2, 'j126Marca', '');
    this._J126Marca_2_5 = import1.J126Marca();
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_2, this._J126Marca_2_5);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.atual;
    if (import14.checkBinding(this._expr_0, currVal_0, 'atual', 'asset:corpus_ngdart/lib/src/j126_tipos_de_diretiva.dart')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._J126Item_0_5, 'valor', currVal_0);
      }
      this._J126Item_0_5.valor = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j126_tipos_de_diretiva.dart:578:593 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.texto;
    if (import14.checkBinding(this._expr_1, currVal_1, 'texto', 'asset:corpus_ngdart/lib/src/j126_tipos_de_diretiva.dart')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._J126Marca_1_5, 'marca', currVal_1);
      }
      this._J126Marca_1_5.marca = currVal_1 /* REF:asset:corpus_ngdart/lib/src/j126_tipos_de_diretiva.dart:625:640 */;
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.texto;
    if (import14.checkBinding(this._expr_2, currVal_2, 'texto', 'asset:corpus_ngdart/lib/src/j126_tipos_de_diretiva.dart')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._J126Marca_2_5, 'marca', currVal_2);
      }
      this._J126Marca_2_5.marca = currVal_2 /* REF:asset:corpus_ngdart/lib/src/j126_tipos_de_diretiva.dart:663:678 */;
      this._expr_2 = currVal_2;
    }
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
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J126Lista, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J126ListaNgFactory = ComponentFactory<import1.J126Lista>('j126-lista', viewFactory_J126ListaHost0);
ComponentFactory<import1.J126Lista> get J126ListaNgFactory {
  return _J126ListaNgFactory;
}

ComponentFactory<import1.J126Lista<T>> createJ126ListaFactory<T>() {
  return ComponentFactory('j126-lista', viewFactory_J126ListaHost0);
}

final List<Object> styles$J126ListaHost = const [];

class _ViewJ126ListaHost0<T> extends import10.HostView<import1.J126Lista<T>> {
  @override
  void build() {
    this.componentView = ViewJ126Lista0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J126Lista();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.J126Lista<T>> viewFactory_J126ListaHost0<T>() {
  return _ViewJ126ListaHost0();
}

final List<Object> styles$J126Usa = const [];

class ViewJ126Usa0 extends import0.ComponentView<import1.J126Usa> {
  late final ViewJ126Lista0<int> _compView_0;
  late final import1.J126Lista<int> _J126Lista_0_5;
  static import3.ComponentStyles? _componentStyles;
  ViewJ126Usa0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j126-usa'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j126_tipos_de_diretiva.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ126Lista0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J126Lista_0_5 = import1.J126Lista();
    this._compView_0.create(this._J126Lista_0_5);
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
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J126Usa, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J126UsaNgFactory = ComponentFactory<import1.J126Usa>('j126-usa', viewFactory_J126UsaHost0);
ComponentFactory<import1.J126Usa> get J126UsaNgFactory {
  return _J126UsaNgFactory;
}

ComponentFactory<import1.J126Usa> createJ126UsaFactory() {
  return ComponentFactory('j126-usa', viewFactory_J126UsaHost0);
}

final List<Object> styles$J126UsaHost = const [];

class _ViewJ126UsaHost0 extends import10.HostView<import1.J126Usa> {
  @override
  void build() {
    this.componentView = ViewJ126Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J126Usa();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.J126Usa> viewFactory_J126UsaHost0() {
  return _ViewJ126UsaHost0();
}
