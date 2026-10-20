set -euo pipefail
snapshot=$(cat /tmp/forge-current-desktop-linux-snapshot)
runuser -u isaque -- python3 /media/psf/ssd_dock/MyRustProjects/dartforge/target/current-desktop-validation/linux-sdk-modes.py "$snapshot"
