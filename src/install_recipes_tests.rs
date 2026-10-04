use super::{openlitespeed_apt_repository_text, prepare_openlitespeed_apt_command};

#[test]
fn resolute_repository_uses_scoped_keyring() {
    let repository = openlitespeed_apt_repository_text("resolute");
    assert!(repository.contains(" resolute main"));
    assert!(repository.contains("signed-by=/usr/share/keyrings/litespeed-archive-keyring.gpg"));
    assert!(!repository.contains("trusted=yes"));
}

#[test]
fn apt_bootstrap_disables_unsigned_source_before_update() {
    let command = prepare_openlitespeed_apt_command();
    let script = command.args.last().copied().unwrap_or_default();
    let disable = script.find("mv -f \"$repo\" \"$disabled\"").unwrap();
    let first_update = script.find("apt-get update -y").unwrap();
    let key_install = script.find("install -m 0644").unwrap();
    let restore = script.rfind("mv -f \"$disabled\" \"$repo\"").unwrap();
    let final_update = script.rfind("apt-get update -y").unwrap();

    assert!(disable < first_update);
    assert!(first_update < key_install);
    assert!(key_install < restore);
    assert!(restore < final_update);
    assert!(script.contains("3E892522DB44E1B063D366C5011AA62DEDA1F085"));
}
