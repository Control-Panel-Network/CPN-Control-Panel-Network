Name:           cpn-installer
Version:        %{?cpn_version}%{!?cpn_version:0.1.0}
Release:        2%{?dist}
Summary:        Instalador de CPN Server Panel
License:        GPL-3.0-only
URL:            https://github.com/KraoESPfan1n/CPN-Control-Panel-Network
Source0:        cpn-installer
Source1:        cpn-panel.tar.gz
Source2:        node-runtime.tar.xz
Source3:        cpn-panel.service

ExclusiveArch:  x86_64
Requires:       systemd
Requires:       dnf
Requires:       curl
Requires:       openssl

%description
Instalador local con interfaz web para preparar CPN Server Panel en AlmaLinux.

%prep

%build

%install
install -Dpm 0755 %{SOURCE0} %{buildroot}%{_bindir}/cpn-installer
mkdir -p %{buildroot}/opt/cpn-panel/app %{buildroot}/opt/cpn-panel/node
tar -xzf %{SOURCE1} -C %{buildroot}/opt/cpn-panel/app
tar -xJf %{SOURCE2} -C %{buildroot}/opt/cpn-panel/node
install -Dpm 0644 %{SOURCE3} %{buildroot}%{_unitdir}/cpn-panel.service
mkdir -p %{buildroot}/opt/cpn-panel/app/.next/cache

%pre
if [ ! -r /etc/os-release ]; then
    echo "CPN no pudo identificar el sistema operativo." >&2
    exit 1
fi
. /etc/os-release
if [ "$ID" != "almalinux" ]; then
    echo "CPN solo se puede instalar en AlmaLinux 9.x; se detectó ${PRETTY_NAME:-un sistema no compatible}." >&2
    exit 1
fi
case "$VERSION_ID" in
    9|9.*) ;;
    *)
        echo "CPN requiere AlmaLinux 9.x; se detectó AlmaLinux ${VERSION_ID:-desconocido}." >&2
        exit 1
        ;;
esac

%post
%systemd_post cpn-panel.service
echo
echo "CPN Installer se instaló correctamente."
echo "Para iniciar el instalador web, ejecuta:"
echo "  sudo cpn-installer"
echo

%preun
%systemd_preun cpn-panel.service

%postun
%systemd_postun_with_restart cpn-panel.service

%files
%{_bindir}/cpn-installer
%{_unitdir}/cpn-panel.service
%dir %attr(0755,root,root) /opt/cpn-panel
%dir %attr(0755,root,root) /opt/cpn-panel/app
/opt/cpn-panel/app/*
/opt/cpn-panel/app/.next
/opt/cpn-panel/node/*

%changelog
* Fri Aug 28 2026 CPN <dev@cpn.invalid> - 0.1.0-2
- Añade la fase Configurando y corrige la validación de OpenLiteSpeed con advertencias.

* Tue Aug 11 2026 CPN <dev@cpn.invalid> - 0.1.0-1
- Primera versión del instalador para AlmaLinux
