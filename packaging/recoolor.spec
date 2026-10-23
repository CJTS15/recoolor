# Built by packaging/build-rpm.sh, which makes both sources from the Git tree.
%global app_id io.github.cjts15.Recoolor
# Release builds are already stripped (Cargo.toml), so there is no debug info to split.
%global debug_package %{nil}

Name:           recoolor
Version:        0.1.1
Release:        1%{?dist}
Summary:        Recolor GNOME and its apps with one palette
License:        GPL-3.0-or-later
URL:            https://github.com/CJTS15/recoolor
Source0:        %{name}-%{version}.tar.gz
# The crates from crates.io, vendored so the build runs offline.
Source1:        %{name}-%{version}-vendor.tar.gz

BuildRequires:  cargo >= 1.85
BuildRequires:  rust >= 1.85
BuildRequires:  gtk4-devel >= 4.16
BuildRequires:  libadwaita-devel >= 1.6
BuildRequires:  make
BuildRequires:  desktop-file-utils
BuildRequires:  appstream
Requires:       gtk4 >= 4.16
Requires:       libadwaita >= 1.6
Recommends:     gnome-shell-extension-user-theme

%description
Pick a palette (Catppuccin, Dracula, Tokyo Night, Gruvbox, Rosé Pine, Nord and
more) and Recoolor recolors GTK and libadwaita apps, GNOME Shell, Flatpak apps,
the accent color, and apps such as Ptyxis, fish, VS Code, Firefox and Spotifast.
Reset puts everything back the way it was.

%prep
%autosetup -a 1
mkdir -p .cargo
cat > .cargo/config.toml <<'CARGO'
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
CARGO

%build
cargo build --release --locked --offline

%install
%make_install PREFIX=%{_prefix}

%check
cargo test --release --locked --offline
desktop-file-validate %{buildroot}%{_datadir}/applications/%{app_id}.desktop
appstreamcli validate --no-net %{buildroot}%{_metainfodir}/%{app_id}.metainfo.xml

%files
%license LICENSE
%doc README.md CHANGELOG.md
%{_bindir}/recoolor
%{_datadir}/applications/%{app_id}.desktop
%{_metainfodir}/%{app_id}.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/%{app_id}.svg
%{_datadir}/icons/hicolor/symbolic/apps/%{app_id}-symbolic.svg

%changelog
* Fri Oct 23 2026 CJ Sabijon <cjsabijon@gmail.com> - 0.1.1-1
- Fix unreadable Shell after GDM Settings used the theme; light Shell in light mode.

* Fri Oct 23 2026 CJ Sabijon <cjsabijon@gmail.com> - 0.1.0-1
- First release.
