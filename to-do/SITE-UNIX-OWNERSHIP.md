# Website Unix ownership (`user:group`)

## Meaning of "CPN user"

The **website Unix account** CPN ensures for each primary site home (`/home/<fqdn>`).
It is not the panel login name, and it is never `root`. Owner and primary group are
the same account (`siteuser:siteuser`). Subdomains inherit the parent's account.

## Create

1. Create home + `public_html` (+ `logs`).
2. Resolve Unix account: existing passwd home match, parent inherit, or `useradd -M -U`.
3. `chown` home inode + docroot tree to `user:user`, mode `0755` (not group/other-writable).
4. Security check verifies uid/gid/mode; failure aborts create (no site JSON).

## Heal

```bash
sudo cpn doctor check --id sites.ownership
sudo cpn doctor heal --id sites.ownership
```

Heal sets registered root-owned homes to the existing website Unix account when
one already owns that home (for example a migrated account), without inventing a
random new user for populated production trees.

## SFTP

Jailed OpenSSH accounts use `/var/lib/cpn/sftp-jails/<username>/` (root-owned chroot).
They must not reset `/home/<fqdn>` back to `root:root`.
