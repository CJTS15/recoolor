#!/usr/bin/env bash
# Builds the Recoolor RPM and source RPM into target/rpmbuild from the committed
# Git tree (HEAD). Needs: rpm-build, cargo, gtk4-devel, libadwaita-devel,
# desktop-file-utils, appstream.
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n1)
spec_version=$(sed -n 's/^Version: *//p' packaging/recoolor.spec)
if [[ "$version" != "$spec_version" ]]; then
    echo "Cargo.toml says $version but recoolor.spec says $spec_version" >&2
    exit 1
fi

top="$PWD/target/rpmbuild"
stage="$PWD/target/vendor-stage"
rm -rf "$top" "$stage"
mkdir -p "$top/SOURCES" "$stage"

git archive --format=tar.gz --prefix="recoolor-$version/" \
    -o "$top/SOURCES/recoolor-$version.tar.gz" HEAD
cargo vendor --locked --versioned-dirs "$stage/vendor" > /dev/null
tar -C "$stage" -czf "$top/SOURCES/recoolor-$version-vendor.tar.gz" vendor

rpmbuild -ba --define "_topdir $top" packaging/recoolor.spec
find "$top/RPMS" "$top/SRPMS" -name '*.rpm'
