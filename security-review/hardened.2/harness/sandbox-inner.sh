#!/bin/sh
set -eu
source=$1
rootdir=$2
tests=$3
shift 3
mount --make-rprivate /
/usr/sbin/ip link set lo up
mount -t tmpfs -o size=64m,mode=0755,nosuid,nodev tmpfs "$rootdir"
mkdir -p "$rootdir/usr" "$rootdir/opt/node" "$rootdir/work" "$rootdir/tests" "$rootdir/tmp" "$rootdir/proc" "$rootdir/dev" "$rootdir/etc"
ln -s usr/bin "$rootdir/bin"
ln -s usr/sbin "$rootdir/sbin"
ln -s usr/lib "$rootdir/lib"
ln -s usr/lib64 "$rootdir/lib64"
mount --bind /usr "$rootdir/usr"
mount -o remount,bind,ro,nosuid,nodev /usr "$rootdir/usr"
mount --bind /opt/nvm/versions/node/v22.16.0 "$rootdir/opt/node"
mount -o remount,bind,ro,nosuid,nodev /opt/nvm/versions/node/v22.16.0 "$rootdir/opt/node"
mount --bind "$source" "$rootdir/work"
mount -o remount,bind,ro,nosuid,nodev "$source" "$rootdir/work"
mount --bind "$tests" "$rootdir/tests"
mount -o remount,bind,ro,nosuid,nodev "$tests" "$rootdir/tests"
for device in null zero random urandom; do
  touch "$rootdir/dev/$device"
  mount --bind "/dev/$device" "$rootdir/dev/$device"
done
mount -t tmpfs -o size=64m,mode=1777,nosuid,nodev tmpfs "$rootdir/tmp"
printf 'root:x:0:0:sandbox:/tmp/home:/bin/sh\n' > "$rootdir/etc/passwd"
printf 'root:x:0:\n' > "$rootdir/etc/group"
mount -o remount,bind,ro "$rootdir"
ulimit -t 45
ulimit -v 8388608
ulimit -f 32768
ulimit -n 128
ulimit -u 96 2>/dev/null || true
exec /usr/sbin/chroot "$rootdir" /usr/bin/setpriv --bounding-set=-all --inh-caps=-all --ambient-caps=-all --no-new-privs /usr/bin/env -i PATH=/opt/node/bin:/usr/bin:/bin HOME=/tmp/home TMPDIR=/tmp LANG=C.UTF-8 INSTALL_SECURITY_TEST_ROOT=/work NODE_OPTIONS="--max-old-space-size=256 --experimental-strip-types --disable-wasm-trap-handler" /bin/sh -c 'cd /work; exec /opt/node/bin/node "$@"' node "$@"
