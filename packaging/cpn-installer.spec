Name:           cpn-installer
Version:        0.1.0
Release:        1%{?dist}
Summary:        Instalador de CPN Server Panel
License:        GPL-3.0-only
URL:            https://github.com/KraoESPfan1n/CPN-Control-Panel-Network
Source0:        cpn-installer

ExclusiveArch:  x86_64 aarch64
Requires:       systemd
Requires:       dnf
Requires:       curl

%description
Instalador local con interfaz web para preparar CPN Server Panel en AlmaLinux.

%prep

%build

%install
install -Dpm 0755 %{SOURCE0} %{buildroot}%{_bindir}/cpn-installer

%files
%{_bindir}/cpn-installer

%changelog
* Tue Aug 11 2026 CPN <dev@cpn.invalid> - 0.1.0-1
- Primera versión del instalador para AlmaLinux
