Name:           cpn-installer
Version:        0.2.0
Release:        1%{?dist}
Summary:        Instalador de CPN Server Panel
License:        GPL-3.0-only
URL:            https://github.com/Control-Panel-Network/CPN-Control-Panel-Network
Source0:        cpn-installer

ExclusiveArch:  x86_64 aarch64
Requires:       systemd
Requires:       dnf
Requires:       curl

%description
Instalador local con interfaz web para preparar CPN Server Panel en invitados Linux alineados con CyberPanel (AlmaLinux/Rocky/RHEL y afines; ver to-do/OS-SUPPORT-MATRIX.md).

%prep

%build

%install
install -Dpm 0755 %{SOURCE0} %{buildroot}%{_bindir}/cpn-installer

%files
%{_bindir}/cpn-installer

%changelog
* Thu Sep 03 2026 CPN <dev@cpn.invalid> - 0.2.0-1
- AlmaLinux 9 and 10, install i18n, first-account setup, panel login post-install

* Thu Sep 03 2026 CPN <dev@cpn.invalid> - 0.1.0-1
- Soporte de empaquetado para AlmaLinux 9 y AlmaLinux 10 (%{dist} el9/el10)

* Tue Aug 11 2026 CPN <dev@cpn.invalid> - 0.1.0-1
- Primera versión del instalador para AlmaLinux
