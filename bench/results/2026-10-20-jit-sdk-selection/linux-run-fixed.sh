set -euo pipefail
runuser -u isaque -- python3 /media/psf/ssd_dock/MyRustProjects/dartforge/target/current-desktop-validation/linux-ui.py jit
runuser -u isaque -- python3 /media/psf/ssd_dock/MyRustProjects/dartforge/target/current-desktop-validation/linux-sdk-modes.py
