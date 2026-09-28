// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j99_filho_que_herda.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j99_filho_que_herda.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;
import 'package:ngdart/src/devtools.dart' as import13;

final List<Object> styles$J99Filho = const [];

class ViewJ99Filho0 extends import0.ComponentView<import1.J99Filho> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  final import2.TextBinding _textBinding_3 = import2.TextBinding();
  Object? _expr_0;
  static import3.ComponentStyles? _componentStyles;
  ViewJ99Filho0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j99-filho'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j99_filho_que_herda.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'b');
    _el_0.append(this._textBinding_1.element);
    final _el_2 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'i');
    _el_2.append(this._textBinding_3.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.titulo)) /* REF:asset:corpus_ngdart/lib/src/j99_filho_que_herda.dart:817:829 */;
    this._textBinding_3.updateText(import9.interpolateString0(_ctx.legenda)) /* REF:asset:corpus_ngdart/lib/src/j99_filho_que_herda.dart:836:849 */;
  }

  void detectHostChanges(bool firstCheck) {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.ativo;
    if (import10.checkBinding(this._expr_0, currVal_0, null, null)) {
      import8.updateClassBindingNonHtml(this.rootElement, 'ativo', currVal_0);
      this._expr_0 = currVal_0;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J99Filho, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J99FilhoNgFactory = ComponentFactory<import1.J99Filho>('j99-filho', viewFactory_J99FilhoHost0);
ComponentFactory<import1.J99Filho> get J99FilhoNgFactory {
  return _J99FilhoNgFactory;
}

ComponentFactory<import1.J99Filho> createJ99FilhoFactory() {
  return ComponentFactory('j99-filho', viewFactory_J99FilhoHost0);
}

final List<Object> styles$J99FilhoHost = const [];

class _ViewJ99FilhoHost0 extends import12.HostView<import1.J99Filho> {
  @override
  void build() {
    this.componentView = ViewJ99Filho0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J99Filho();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if (((!import10.debugThrowIfChanged) && firstCheck)) {
      this.component.ngOnInit();
    }
    this.componentView.detectHostChanges(firstCheck);
    this.componentView.detectChanges();
    if ((!import10.debugThrowIfChanged)) {
      if (firstCheck) {
        this.component.ngAfterViewInit();
      }
    }
  }

  @override
  void destroyInternal() {
    this.component.ngOnDestroy();
  }
}

import12.HostView<import1.J99Filho> viewFactory_J99FilhoHost0() {
  return _ViewJ99FilhoHost0();
}

final List<Object> styles$J99Usa = const [];

class ViewJ99Usa0 extends import0.ComponentView<import1.J99Usa> {
  late final ViewJ99Filho0 _compView_0;
  late final import1.J99Filho _J99Filho_0_5;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_3;
  static import3.ComponentStyles? _componentStyles;
  ViewJ99Usa0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j99-usa'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j99_filho_que_herda.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ99Filho0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    import8.setAttribute(_el_0, 'rotulo', 'fixo');
    this._J99Filho_0_5 = import1.J99Filho();
    this._compView_0.create(this._J99Filho_0_5);
    final subscription_0 = this._J99Filho_0_5.mudou.listen(this.eventHandler1(_ctx.ouvir));
    final subscription_1 = this._J99Filho_0_5.fechar.listen(this.eventHandler0(_ctx.fechou));
    this.initSubscriptions([subscription_0, subscription_1]);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    if (firstCheck) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._J99Filho_0_5, 'rotulo', 'fixo');
      }
      this._J99Filho_0_5.legenda = 'fixo' /* REF:asset:corpus_ngdart/lib/src/j99_filho_que_herda.dart:1079:1092 */;
    }
    final currVal_0 = _ctx.ligado;
    if (import10.checkBinding(this._expr_0, currVal_0, 'ligado', 'asset:corpus_ngdart/lib/src/j99_filho_que_herda.dart')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._J99Filho_0_5, 'desligado', currVal_0);
      }
      this._J99Filho_0_5.desligado = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j99_filho_que_herda.dart:1093:1113 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.nome;
    if (import10.checkBinding(this._expr_1, currVal_1, 'nome', 'asset:corpus_ngdart/lib/src/j99_filho_que_herda.dart')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._J99Filho_0_5, 'titulo', currVal_1);
      }
      this._J99Filho_0_5.titulo = currVal_1 /* REF:asset:corpus_ngdart/lib/src/j99_filho_que_herda.dart:1063:1078 */;
      this._expr_1 = currVal_1;
    }
    final currVal_3 = _ctx.n;
    if (import10.checkBinding(this._expr_3, currVal_3, 'n', 'asset:corpus_ngdart/lib/src/j99_filho_que_herda.dart')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._J99Filho_0_5, 'contador', currVal_3);
      }
      this._J99Filho_0_5.contador = currVal_3 /* REF:asset:corpus_ngdart/lib/src/j99_filho_que_herda.dart:1114:1128 */;
      this._expr_3 = currVal_3;
    }
    if (((!import10.debugThrowIfChanged) && firstCheck)) {
      this._J99Filho_0_5.ngOnInit();
    }
    this._compView_0.detectHostChanges(firstCheck);
    this._compView_0.detectChanges();
    if ((!import10.debugThrowIfChanged)) {
      if (firstCheck) {
        this._J99Filho_0_5.ngAfterViewInit();
      }
    }
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._J99Filho_0_5.ngOnDestroy();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J99Usa, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J99UsaNgFactory = ComponentFactory<import1.J99Usa>('j99-usa', viewFactory_J99UsaHost0);
ComponentFactory<import1.J99Usa> get J99UsaNgFactory {
  return _J99UsaNgFactory;
}

ComponentFactory<import1.J99Usa> createJ99UsaFactory() {
  return ComponentFactory('j99-usa', viewFactory_J99UsaHost0);
}

final List<Object> styles$J99UsaHost = const [];

class _ViewJ99UsaHost0 extends import12.HostView<import1.J99Usa> {
  @override
  void build() {
    this.componentView = ViewJ99Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J99Usa();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.J99Usa> viewFactory_J99UsaHost0() {
  return _ViewJ99UsaHost0();
}
