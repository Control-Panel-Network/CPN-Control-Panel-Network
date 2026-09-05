# CPN installer runtime image (AlmaLinux + systemd) for development/smoke testing.
# Privileged mode and cgroup mounts are required at run time; see scripts/docker-run.sh.

ARG ALMA_VERSION=9
FROM almalinux:${ALMA_VERSION}

ARG ALMA_VERSION=9
LABEL org.opencontainers.image.title="CPN Installer"
LABEL org.opencontainers.image.description="CPN Control Panel Network web installer on AlmaLinux"
LABEL org.opencontainers.image.source="https://github.com/Control-Panel-Network/CPN-Control-Panel-Network"
LABEL cpn.almalinux.version="${ALMA_VERSION}"

ENV container=docker

# AlmaLinux base images ship curl-minimal; requesting curl without --allowerasing fails.
RUN dnf -y update \
  && dnf -y install --allowerasing systemd systemd-udev dnf-plugins-core curl which hostname \
  && dnf clean all \
  && rm -f /lib/systemd/system/multi-user.target.wants/* \
  && rm -f /etc/systemd/system/*.wants/* \
  && rm -f /lib/systemd/system/local-fs.target.wants/* \
  && rm -f /lib/systemd/system/sockets.target.wants/*udev* \
  && rm -f /lib/systemd/system/sockets.target.wants/*initctl* \
  && rm -f /lib/systemd/system/basic.target.wants/* \
  && rm -f /lib/systemd/system/anaconda.target.wants/* \
  && systemctl set-default multi-user.target

# Build context must include a single RPM named cpn-installer.rpm.
# scripts/docker-run.sh stages the built RPM before docker build.
COPY cpn-installer.rpm /tmp/cpn-installer.rpm
RUN dnf -y install /tmp/cpn-installer.rpm \
  && rm -f /tmp/cpn-installer.rpm \
  && dnf clean all \
  && systemctl enable cpn-installer.service

EXPOSE 2087
STOPSIGNAL SIGRTMIN+3
CMD ["/usr/lib/systemd/systemd"]
