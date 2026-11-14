#!/bin/sh
set -eu

here="$(cd "$(dirname "$0")" && pwd)"

device_cflags="-mcpu=cortex-a53"

usage() {
	echo "usage: $0 stamp COMMIT | build COMMIT TARBALL WORKDIR OUT" >&2
	exit 2
}

sha256() {
	if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi |
		cut -d' ' -f1
}

stamp() {
	echo "commit=$1"
	echo "source=https://github.com/libretro/SameBoy/archive/$1.tar.gz"
	echo "recipe=make -C libretro platform=unix"
	echo "device_cflags=$device_cflags"
	for p in "$here"/*.patch; do
		[ -f "$p" ] || continue
		echo "patch=$(basename "$p") sha256:$(sha256 "$p")"
	done
}

# SameBoy's libretro makefile regenerates the boot ROM C arrays with hexdump, which the device
# build box does not carry. This is the same transformation the makefile falls back to on Haiku.
hexdump_shim() {
	cat >"$1" <<'EOF'
#!/bin/sh
# hexdump -v -e '/1 "0x%02x, "' FILE
for arg in "$@"; do file="$arg"; done
od -A none -t x1 -v "$file" | sed -e 's/^ /0x/' -e 's/ /, 0x/g' -e 's/$/,/g' | tr '\n' ' '
EOF
	chmod +x "$1"
}

build() {
	commit="$1" tarball="$2" work="$3" out="$4"
	src="$work/SameBoy-$commit"

	rm -rf "$work"
	mkdir -p "$work"
	tar -xzf "$tarball" -C "$work"
	if [ ! -f "$src/libretro/Makefile" ]; then
		echo "$tarball does not hold SameBoy-$commit/libretro/Makefile" >&2
		exit 1
	fi

	for p in "$here"/*.patch; do
		[ -f "$p" ] || continue
		patch -p1 --batch --forward -d "$src" <"$p"
	done

	path="$PATH"
	if ! command -v hexdump >/dev/null 2>&1; then
		# make runs in libretro/, so the shim has to be an absolute path.
		shim="$(cd "$work" && pwd)/hexdump"
		hexdump_shim "$shim"
		path="$(dirname "$shim"):$path"
	fi

	version="$(sed -n 's/^VERSION[[:space:]]*:=[[:space:]]*//p' "$src/version.mk")"
	PATH="$path" VERSION="$version" CFLAGS="$device_cflags" \
		make -C "$src/libretro" platform=unix -j"$(getconf _NPROCESSORS_ONLN)"

	mkdir -p "$(dirname "$out")"
	cp "$src/libretro/sameboy_libretro.so" "$out"
	stamp "$commit" >"$out.meta"
}

case "${1:-}" in
stamp)
	[ $# -eq 2 ] && [ -n "$2" ] || usage
	stamp "$2"
	;;
build)
	[ $# -eq 5 ] && [ -n "$2" ] && [ -n "$3" ] && [ -n "$4" ] && [ -n "$5" ] || usage
	build "$2" "$3" "$4" "$5"
	;;
*)
	usage
	;;
esac
