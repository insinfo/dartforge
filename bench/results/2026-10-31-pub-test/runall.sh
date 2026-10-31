#!/bin/bash
# Differential `pub get` on every project of /Volumes/ssd_dock/MyProjectsDart,
# first without the lockfiles (fresh resolution), then with them.
PT=/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad/pt
cd "$PT"
P="access_to_dart aggr angular argon2 asmjit canvas-editor-port canvas_text_editor coreutils dart_graphics dart_ocr dart_odf dart_quill dart_ui dgfx docx_dart docx_rendering dpdf dpgsql eloquent_dart insinfo_dart_pdf jbig2 jose jpeg2000 jsPDF limitless_ui mysql.dart oracledb_dart pdf.js pdfbox_dart pdfium_bindings pdfium_dart pg8000 playwright-connect playwright-init playwright-test-fixtures playwright-test-server playwright-tracing playwright-video-api playwright-video-cast playwright-video-mux playwright poe popper_dart portal_servicos postgres_fork sqlite_dart taleweaver tlslite xlsx_editor zap_bot new_sali/backend"
KEEP_LOCK=0 python3 -u pubreal.py all-nolock "$@" -- $P > all-nolock.log 2>&1
KEEP_LOCK=1 python3 -u pubreal.py all-lock "$@" -- $P > all-lock.log 2>&1
