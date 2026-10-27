# Built by packaging/build-rpm.sh, which makes both sources from the Git tree.
%global app_id io.github.cjts15.Recoolor
# Release builds are already stripped (Cargo.toml), so there is no debug info to split.
%global debug_package %{nil}

Name:           recoolor
Version:        1.3.0
Release:        1%{?dist}
Summary:        Recolor GNOME and its apps with one palette
License:        GPL-3.0-or-later
URL:            https://github.com/CJTS15/recoolor
Source0:        %{name}-%{version}.tar.gz
# The crates from crates.io, vendored so the build runs offline.
Source1:        %{name}-%{version}-vendor.tar.gz

BuildRequires:  cargo >= 1.89
BuildRequires:  rust >= 1.89
BuildRequires:  gtk4-devel >= 4.16
BuildRequires:  libadwaita-devel >= 1.6
BuildRequires:  make
BuildRequires:  desktop-file-utils
BuildRequires:  appstream
Requires:       gtk4 >= 4.16
Requires:       libadwaita >= 1.6
Recommends:     gnome-shell-extension-user-theme
Recommends:     adw-gtk3-theme

%description
Pick a palette (Catppuccin, Dracula, Tokyo Night, Gruvbox, Rosé Pine, Nord and
more) and Recoolor recolors GTK and libadwaita apps, GNOME Shell, Qt and Flatpak
apps, the wallpaper and lock screen, the accent color, and apps such as Ptyxis,
fish, Text Editor, VS Code, Firefox and Thunderbird. Reset puts everything back
the way it was.

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
* Tue Oct 27 2026 CJ Sabijon <cjsabijon@gmail.com> - 1.3.0-1
- 179 palettes from Gogh with search; Alt+Tab titles and toasts in the palette.

* Mon Oct 26 2026 CJ Sabijon <cjsabijon@gmail.com> - 1.2.0-1
- Firefox and Thunderbird follow light/dark live; Shell popups follow Blur my Shell.

* Mon Oct 26 2026 CJ Sabijon <cjsabijon@gmail.com> - 1.1.0-1
- Smooth light/dark switches, light style fixes, SearchLightNG, About menu.

* Mon Oct 26 2026 CJ Sabijon <cjsabijon@gmail.com> - 1.0.1-1
- VS Code follows every Apply and light/dark switch; Blur my Shell popups take the palette.

* Mon Oct 26 2026 CJ Sabijon <cjsabijon@gmail.com> - 1.0.0-1
- btop. First stable release.

* Mon Oct 26 2026 CJ Sabijon <cjsabijon@gmail.com> - 0.2.0-1
- Wallpapers and lock screen, adw-gtk3, Text Editor, Thunderbird, Qt and KDE apps, Restart Now.

* Fri Oct 23 2026 CJ Sabijon <cjsabijon@gmail.com> - 0.1.2-1
- Accent picker, Calendar restart, selection highlights, translucent popups with Blur my Shell.

* Fri Oct 23 2026 CJ Sabijon <cjsabijon@gmail.com> - 0.1.1-1
- Fix unreadable Shell after GDM Settings used the theme; light Shell in light mode.

* Fri Oct 23 2026 CJ Sabijon <cjsabijon@gmail.com> - 0.1.0-1
- First release.
