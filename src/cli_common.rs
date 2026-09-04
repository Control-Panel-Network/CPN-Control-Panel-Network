//! Shared CLI helpers (root checks, password input, confirmation).

use std::io::{self, Read, Write};

pub fn is_root() -> bool {
    #[cfg(unix)]
    {
        // SAFETY: geteuid is a pure query of the process credentials.
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

pub fn require_root_for_mutation() -> Result<(), String> {
    if is_root() {
        return Ok(());
    }
    if std::env::var("CPN_ALLOW_NONROOT").ok().as_deref() == Some("1") {
        return Ok(());
    }
    Err(
        "This command requires root. Re-run with sudo, or set CPN_ALLOW_NONROOT=1 for a lab data dir."
            .into(),
    )
}

pub fn read_password(
    password_stdin: bool,
    generate: bool,
) -> Result<(Option<String>, bool), String> {
    if generate && password_stdin {
        return Err("Use either --generate or --password-stdin, not both".into());
    }
    if generate {
        return Ok((None, true));
    }
    if password_stdin {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .map_err(|error| format!("Failed to read password from stdin: {error}"))?;
        let password = buf.trim_end_matches(['\r', '\n']).to_string();
        if password.is_empty() {
            return Err("Password from stdin was empty".into());
        }
        return Ok((Some(password), false));
    }
    eprint!("Password: ");
    let _ = io::stderr().flush();
    let password =
        rpassword::read_password().map_err(|error| format!("Failed to read password: {error}"))?;
    if password.is_empty() {
        return Err("Password was empty (use --generate to create one)".into());
    }
    Ok((Some(password), false))
}

pub fn confirm_delete(prompt: &str, yes: bool) -> Result<(), String> {
    if yes {
        return Ok(());
    }
    eprint!("{prompt} Type YES to confirm: ");
    let _ = io::stderr().flush();
    let mut line = String::new();
    io::stdin()
        .read_line(&mut line)
        .map_err(|error| format!("Failed to read confirmation: {error}"))?;
    if line.trim() == "YES" {
        Ok(())
    } else {
        Err("Aborted (confirmation was not YES)".into())
    }
}

pub fn print_generated(password: Option<String>) -> Result<(), String> {
    let Some(value) = password else {
        return Ok(());
    };
    let path =
        std::env::temp_dir().join(format!("cpn-generated-password-{}.txt", std::process::id()));
    {
        use std::io::Write;
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&path)
            .map_err(|error| format!("Failed to write generated password file: {error}"))?;
        file.write_all(value.as_bytes())
            .map_err(|error| format!("Failed to write generated password file: {error}"))?;
        file.write_all(b"\n")
            .map_err(|error| format!("Failed to write generated password file: {error}"))?;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    println!("generated_password_file={}", path.display());
    Ok(())
}
