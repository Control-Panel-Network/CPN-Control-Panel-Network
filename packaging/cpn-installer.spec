Name:           cpn-installer
Version:        0.2.2
Release:        0.alpha7%{?dist}
Summary:        Instalador de CPN Server Panel
License:        GPL-3.0-only
URL:            https://github.com/Control-Panel-Network/CPN-Control-Panel-Network
Source0:        cpn-installer
Source1:        cpn
Source2:        cpn-installer.service

ExclusiveArch:  x86_64 aarch64
Requires:       systemd
Requires:       dnf
Requires:       curl
Requires:       ca-certificates

%description
Instalador local con interfaz web para preparar CPN Server Panel en sistemas Linux de la familia Enterprise Linux compatibles con CPN.
Incluye la CLI de operador cpn para administración desde SSH.

%prep

%build

%install
install -Dpm 0755 %{SOURCE0} %{buildroot}%{_bindir}/cpn-installer
install -Dpm 0755 %{SOURCE1} %{buildroot}%{_bindir}/cpn
install -Dpm 0644 %{SOURCE2} %{buildroot}/usr/lib/systemd/system/cpn-installer.service

%post
systemctl daemon-reload >/dev/null 2>&1 || :

%postun
systemctl daemon-reload >/dev/null 2>&1 || :

%files
%{_bindir}/cpn-installer
%{_bindir}/cpn
/usr/lib/systemd/system/cpn-installer.service

%changelog
* Thu Sep 03 2026 CPN <dev@cpn.invalid> - 0.2.0-1
- Add cpn operator CLI (account and site management over SSH)
- AlmaLinux 9 and 10, install i18n, first-account setup, panel login post-install

* Thu Sep 03 2026 CPN <dev@cpn.invalid> - 0.1.0-1
- Soporte de empaquetado para AlmaLinux 9 y AlmaLinux 10 (%{dist} el9/el10)

* Tue Aug 11 2026 CPN <dev@cpn.invalid> - 0.1.0-1
- Primera versión del instalador para AlmaLinux
