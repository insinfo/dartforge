#!/bin/bash
# Starts the fake pub servers with failure modes on ports 8766..8774.
SP=/private/tmp/claude-501/-Volumes-ssd-dock-MyRustProjects-dartforge/ac169d57-450d-4d30-a335-fc76af999313/scratchpad
python3 -I "$SP/pp/bin/fakepub.py" 8765 ok "$SP/pp/server" &
port=8766
for mode in auth401 auth403 new500 newbad gcs noloc finerr finbad newnourl; do
  python3 -I "$SP/pp/bin/fakepub.py" $port $mode "$SP/pp/server" &
  port=$((port+1))
done
wait
