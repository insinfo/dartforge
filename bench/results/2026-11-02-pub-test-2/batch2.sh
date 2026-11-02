#!/bin/bash
# The second evidence batch: test reporters (part 2), cache2 and global scenarios again.
. "$(dirname "$0")/env.sh"
bash "$SP/bin/reporters2.sh"
echo "#### cache2"
bash "$SP/bin/pubscen.sh" cache2 "$SP/pc/cache2.txt"
bash "$SP/bin/cachefiles.sh" cache2
echo "#### global1"
bash "$SP/bin/pubscen.sh" global1 "$SP/pc/global1.txt"
bash "$SP/bin/cachefiles.sh" global1
echo "#### global2"
bash "$SP/bin/pubscen.sh" global2 "$SP/pc/global2.txt"
bash "$SP/bin/cachefiles.sh" global2
