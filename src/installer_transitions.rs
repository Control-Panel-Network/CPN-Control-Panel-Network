//! Authoritative installer state transitions (issue #20).

use crate::model::InstallerStatus;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionDenied {
    pub message: &'static str,
}

fn busy(phase: &str) -> bool {
    matches!(
        phase,
        "configuring" | "downloading" | "installing" | "testing"
    )
}

/// Whether `/api/install/server` may start from the current status.
pub fn can_start_server(
    status: &InstallerStatus,
    force_reinstall: bool,
) -> Result<(), TransitionDenied> {
    if busy(status.phase) {
        return Err(TransitionDenied {
            message: "Ya hay una instalacion en curso",
        });
    }
    if status.server_ready && !force_reinstall {
        return Err(TransitionDenied {
            message: "El servidor web ya esta instalado. Envia force_reinstall=true o usa repair/upgrade.",
        });
    }
    if !matches!(
        status.phase,
        "ready" | "completed" | "failed" | "maintenance"
    ) {
        return Err(TransitionDenied {
            message: "Transicion no valida para instalar el servidor",
        });
    }
    Ok(())
}

/// Whether `/api/install/mail` may start from the current status.
pub fn can_start_mail(
    status: &InstallerStatus,
    force_reinstall: bool,
) -> Result<(), TransitionDenied> {
    if busy(status.phase) {
        return Err(TransitionDenied {
            message: "Ya hay una instalacion en curso",
        });
    }
    if !status.server_ready {
        return Err(TransitionDenied {
            message: "Instala y verifica el servidor web antes del correo",
        });
    }
    if status.selected_mail.is_some() && status.phase == "completed" && !force_reinstall {
        return Err(TransitionDenied {
            message: "El correo ya esta instalado. Envia force_reinstall=true para cambiar de receta.",
        });
    }
    if !matches!(
        status.phase,
        "completed" | "failed" | "maintenance" | "ready"
    ) {
        return Err(TransitionDenied {
            message: "Transicion no valida para instalar el correo",
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{InstallerStatus, MailSystem, ServerEngine};

    fn base() -> InstallerStatus {
        InstallerStatus::default()
    }

    #[test]
    fn mail_requires_server_ready() {
        let status = base();
        assert!(can_start_mail(&status, false).is_err());
    }

    #[test]
    fn mail_allowed_after_server() {
        let mut status = base();
        status.server_ready = true;
        status.phase = "completed";
        status.selected_server = Some(ServerEngine::Nginx);
        assert!(can_start_mail(&status, false).is_ok());
    }

    #[test]
    fn server_reinstall_requires_force() {
        let mut status = base();
        status.server_ready = true;
        status.phase = "completed";
        assert!(can_start_server(&status, false).is_err());
        assert!(can_start_server(&status, true).is_ok());
    }

    #[test]
    fn mail_swap_requires_force() {
        let mut status = base();
        status.server_ready = true;
        status.selected_mail = Some(MailSystem::Roundcube);
        status.phase = "completed";
        assert!(can_start_mail(&status, false).is_err());
        assert!(can_start_mail(&status, true).is_ok());
    }

    #[test]
    fn busy_phases_conflict() {
        let mut status = base();
        status.server_ready = true;
        for phase in ["configuring", "downloading", "installing", "testing"] {
            status.phase = phase;
            assert!(can_start_server(&status, true).is_err(), "{phase}");
            assert!(can_start_mail(&status, true).is_err(), "{phase}");
        }
    }
}
