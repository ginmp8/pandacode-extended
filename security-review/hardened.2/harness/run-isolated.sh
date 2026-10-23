#!/bin/bash
set -eu
source=$(realpath "$1")
shift
rootdir=$(mktemp -d /tmp/pandacode-review-root.XXXXXX)
chown 65534:65534 "$rootdir"
trap 'rmdir "$rootdir" 2>/dev/null || true' EXIT
exec_args=(/usr/bin/timeout --signal=TERM --kill-after=5s 90s /usr/bin/setpriv --reuid=65534 --regid=65534 --clear-groups /usr/bin/unshare --user --map-root-user --mount --pid --net --fork /bin/bash /mnt/data/pandacode-review/tools/sandbox-inner.sh "$source" "$rootdir" /mnt/data/pandacode-review/regression "$@")
"${exec_args[@]}"
