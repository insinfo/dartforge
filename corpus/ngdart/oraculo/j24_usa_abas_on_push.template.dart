// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j24_usa_abas_on_push.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j24_usa_abas_on_push.dart' as import1;
import 'j23_abas_on_push.template.dart' as import2;
import 'j23_abas_on_push.dart' as import3;
import 'd07_filho_on_push.template.dart' as import4;
import 'd07_filho_on_push.dart' as import5;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'package:ngdart/src/utilities.dart' as import9;
import 'dart:html' as import10;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/src/devtools.dart' as import12;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import14;

final List<Object> styles$J24UsaAbasOnPush = const [];

class ViewJ24UsaAbasOnPush0 extends import0.ComponentView<import1.J24UsaAbasOnPush> {
  late final import2.ViewJ23AbasOnPush0 _compView_0;
  late final import3.J23AbasOnPush _J23AbasOnPush_0_5;
  late final import4.ViewD07FilhoOnPush0 _compView_1;
  late final import5.D07FilhoOnPush _D07FilhoOnPush_1_5;
  late final import4.ViewD07FilhoOnPush0 _compView_2;
  late final import5.D07FilhoOnPush _D07FilhoOnPush_2_5;
  static import6.ComponentStyles? _componentStyles;
  ViewJ24UsaAbasOnPush0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import10.document.createElement('j24-usa-abas-on-push'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/j24_usa_abas_on_push.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewJ23AbasOnPush0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J23AbasOnPush_0_5 = import3.J23AbasOnPush();
    this._compView_1 = import4.ViewD07FilhoOnPush0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    import11.setAttribute(_el_1, 'rotulo', 'a');
    this._D07FilhoOnPush_1_5 = import5.D07FilhoOnPush();
    this._compView_1.create(this._D07FilhoOnPush_1_5);
    this._compView_2 = import4.ViewD07FilhoOnPush0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    import11.setAttribute(_el_2, 'rotulo', 'b');
    this._D07FilhoOnPush_2_5 = import5.D07FilhoOnPush();
    this._compView_2.create(this._D07FilhoOnPush_2_5);
    import7.View.queryChangeDetectorRefs[this._D07FilhoOnPush_1_5] = this._compView_1;
    import7.View.queryChangeDetectorRefs[this._D07FilhoOnPush_2_5] = this._compView_2;
    this._J23AbasOnPush_0_5.abas = [this._D07FilhoOnPush_1_5, this._D07FilhoOnPush_2_5];
    import7.View.queryChangeDetectorRefs[this._D07FilhoOnPush_1_5] = this._compView_1;
    this._J23AbasOnPush_0_5.primeira = this._D07FilhoOnPush_1_5;
    this._compView_0.createAndProject(this._J23AbasOnPush_0_5, [
      <Object>[_el_1, _el_2]
    ]);
  }

  @override
  void detectChangesInternal() {
    bool changed = false;
    bool firstCheck = this.firstCheck;
    changed = false;
    if (firstCheck) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._D07FilhoOnPush_1_5, 'rotulo', 'a');
      }
      this._D07FilhoOnPush_1_5.rotulo = 'a' /* REF:package:corpus_ngdart/src/j24_usa_abas_on_push.html:40:50 */;
      changed = true;
    }
    if (changed) {
      this._compView_1.markAsCheckOnce();
    }
    changed = false;
    if (firstCheck) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._D07FilhoOnPush_2_5, 'rotulo', 'b');
      }
      this._D07FilhoOnPush_2_5.rotulo = 'b' /* REF:package:corpus_ngdart/src/j24_usa_abas_on_push.html:93:103 */;
      changed = true;
    }
    if (changed) {
      this._compView_2.markAsCheckOnce();
    }
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
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$J24UsaAbasOnPush, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J24UsaAbasOnPushNgFactory = ComponentFactory<import1.J24UsaAbasOnPush>('j24-usa-abas-on-push', viewFactory_J24UsaAbasOnPushHost0);
ComponentFactory<import1.J24UsaAbasOnPush> get J24UsaAbasOnPushNgFactory {
  return _J24UsaAbasOnPushNgFactory;
}

ComponentFactory<import1.J24UsaAbasOnPush> createJ24UsaAbasOnPushFactory() {
  return ComponentFactory('j24-usa-abas-on-push', viewFactory_J24UsaAbasOnPushHost0);
}

final List<Object> styles$J24UsaAbasOnPushHost = const [];

class _ViewJ24UsaAbasOnPushHost0 extends import14.HostView<import1.J24UsaAbasOnPush> {
  @override
  void build() {
    this.componentView = ViewJ24UsaAbasOnPush0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J24UsaAbasOnPush();
    this.initRootNode(_el_0);
  }
}

import14.HostView<import1.J24UsaAbasOnPush> viewFactory_J24UsaAbasOnPushHost0() {
  return _ViewJ24UsaAbasOnPushHost0();
}
