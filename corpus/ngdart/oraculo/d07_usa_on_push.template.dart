// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'd07_usa_on_push.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'd07_usa_on_push.dart' as import1;
import 'd07_filho_on_push.template.dart' as import2;
import 'd07_filho_on_push.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/devtools.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$D07UsaOnPush = const [];

class ViewD07UsaOnPush0 extends import0.ComponentView<import1.D07UsaOnPush> {
  late final import2.ViewD07FilhoOnPush0 _compView_0;
  late final import3.D07FilhoOnPush _D07FilhoOnPush_0_5;
  late final import2.ViewD07FilhoOnPush0 _compView_1;
  late final import3.D07FilhoOnPush _D07FilhoOnPush_1_5;
  static import4.ComponentStyles? _componentStyles;
  ViewD07UsaOnPush0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('d07-usa-on-push'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/d07_usa_on_push.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewD07FilhoOnPush0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this.updateChildClassNonHtml(_el_0, 'a b');
    import9.setAttribute(_el_0, 'id', 'p1');
    import9.setAttribute(_el_0, 'rotulo', 'fixo');
    this._D07FilhoOnPush_0_5 = import3.D07FilhoOnPush();
    this._compView_0.create(this._D07FilhoOnPush_0_5);
    this._compView_1 = import2.ViewD07FilhoOnPush0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    parentRenderNode.append(_el_1);
    this._D07FilhoOnPush_1_5 = import3.D07FilhoOnPush();
    this._compView_1.create(this._D07FilhoOnPush_1_5);
    import5.View.queryChangeDetectorRefs[this._D07FilhoOnPush_0_5] = this._compView_0;
    _ctx.painel = this._D07FilhoOnPush_0_5;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool changed = false;
    bool firstCheck = this.firstCheck;
    changed = false;
    if (firstCheck) {
      if (import10.isDevToolsEnabled) {
        import10.Inspector.instance.recordInput(this._D07FilhoOnPush_0_5, 'rotulo', 'fixo');
      }
      this._D07FilhoOnPush_0_5.rotulo = 'fixo' /* REF:package:corpus_ngdart/src/d07_usa_on_push.html:47:60 */;
      changed = true;
    }
    if (changed) {
      this._compView_0.markAsCheckOnce();
    }
    changed = false;
    if (firstCheck) {
      if ((_ctx.constante != null)) {
        if (import10.isDevToolsEnabled) {
          import10.Inspector.instance.recordInput(this._D07FilhoOnPush_1_5, 'rotulo', _ctx.constante);
        }
        this._D07FilhoOnPush_1_5.rotulo = _ctx.constante /* REF:package:corpus_ngdart/src/d07_usa_on_push.html:101:121 */;
        changed = true;
      }
    }
    if (changed) {
      this._compView_1.markAsCheckOnce();
    }
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$D07UsaOnPush, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _D07UsaOnPushNgFactory = ComponentFactory<import1.D07UsaOnPush>('d07-usa-on-push', viewFactory_D07UsaOnPushHost0);
ComponentFactory<import1.D07UsaOnPush> get D07UsaOnPushNgFactory {
  return _D07UsaOnPushNgFactory;
}

ComponentFactory<import1.D07UsaOnPush> createD07UsaOnPushFactory() {
  return ComponentFactory('d07-usa-on-push', viewFactory_D07UsaOnPushHost0);
}

final List<Object> styles$D07UsaOnPushHost = const [];

class _ViewD07UsaOnPushHost0 extends import12.HostView<import1.D07UsaOnPush> {
  @override
  void build() {
    this.componentView = ViewD07UsaOnPush0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.D07UsaOnPush();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.D07UsaOnPush> viewFactory_D07UsaOnPushHost0() {
  return _ViewD07UsaOnPushHost0();
}
