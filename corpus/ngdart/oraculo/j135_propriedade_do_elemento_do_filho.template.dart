// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j135_propriedade_do_elemento_do_filho.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j135_propriedade_do_elemento_do_filho.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/interpolate.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;
import 'package:ngdart/src/runtime/check_binding.dart' as import11;
import 'package:ngdart/src/devtools.dart' as import12;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import13;

final List<Object> styles$J135Botao = const [];

class ViewJ135Botao0 extends import0.ComponentView<import1.J135Botao> {
  final import2.TextBinding _textBinding_0 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewJ135Botao0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j135-botao'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j135_propriedade_do_elemento_do_filho.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    parentRenderNode.append(this._textBinding_0.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_0.updateText(import8.interpolateString0(_ctx.rotulo)) /* REF:asset:corpus_ngdart/lib/src/j135_propriedade_do_elemento_do_filho.dart:360:370 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J135Botao, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J135BotaoNgFactory = ComponentFactory<import1.J135Botao>('j135-botao', viewFactory_J135BotaoHost0);
ComponentFactory<import1.J135Botao> get J135BotaoNgFactory {
  return _J135BotaoNgFactory;
}

ComponentFactory<import1.J135Botao> createJ135BotaoFactory() {
  return ComponentFactory('j135-botao', viewFactory_J135BotaoHost0);
}

final List<Object> styles$J135BotaoHost = const [];

class _ViewJ135BotaoHost0 extends import10.HostView<import1.J135Botao> {
  @override
  void build() {
    this.componentView = ViewJ135Botao0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J135Botao();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.J135Botao> viewFactory_J135BotaoHost0() {
  return _ViewJ135BotaoHost0();
}

final List<Object> styles$J135Usa = const [];

class ViewJ135Usa0 extends import0.ComponentView<import1.J135Usa> {
  late final ViewJ135Botao0 _compView_0;
  late final import1.J135Botao _J135Botao_0_5;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  late final import7.HtmlElement _el_0;
  static import3.ComponentStyles? _componentStyles;
  ViewJ135Usa0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j135-usa'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j135_propriedade_do_elemento_do_filho.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ135Botao0(this, 0);
    this._el_0 = this._compView_0.rootElement;
    parentRenderNode.append(this._el_0);
    this._J135Botao_0_5 = import1.J135Botao();
    this._compView_0.create(this._J135Botao_0_5);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_2 = _ctx.texto;
    if (import11.checkBinding(this._expr_2, currVal_2, 'texto', 'asset:corpus_ngdart/lib/src/j135_propriedade_do_elemento_do_filho.dart')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._J135Botao_0_5, 'rotulo', currVal_2);
      }
      this._J135Botao_0_5.rotulo = currVal_2 /* REF:asset:corpus_ngdart/lib/src/j135_propriedade_do_elemento_do_filho.dart:499:515 */;
      this._expr_2 = currVal_2;
    }
    final currVal_0 = _ctx.ident;
    if (import11.checkBinding(this._expr_0, currVal_0, 'ident', 'asset:corpus_ngdart/lib/src/j135_propriedade_do_elemento_do_filho.dart')) {
      import13.setProperty(this._el_0, 'id', currVal_0) /* REF:asset:corpus_ngdart/lib/src/j135_propriedade_do_elemento_do_filho.dart:486:498 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.texto;
    if (import11.checkBinding(this._expr_1, currVal_1, 'texto', 'asset:corpus_ngdart/lib/src/j135_propriedade_do_elemento_do_filho.dart')) {
      import13.setProperty(this._el_0, 'title', currVal_1) /* REF:asset:corpus_ngdart/lib/src/j135_propriedade_do_elemento_do_filho.dart:516:531 */;
      this._expr_1 = currVal_1;
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
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J135Usa, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J135UsaNgFactory = ComponentFactory<import1.J135Usa>('j135-usa', viewFactory_J135UsaHost0);
ComponentFactory<import1.J135Usa> get J135UsaNgFactory {
  return _J135UsaNgFactory;
}

ComponentFactory<import1.J135Usa> createJ135UsaFactory() {
  return ComponentFactory('j135-usa', viewFactory_J135UsaHost0);
}

final List<Object> styles$J135UsaHost = const [];

class _ViewJ135UsaHost0 extends import10.HostView<import1.J135Usa> {
  @override
  void build() {
    this.componentView = ViewJ135Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J135Usa();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.J135Usa> viewFactory_J135UsaHost0() {
  return _ViewJ135UsaHost0();
}
