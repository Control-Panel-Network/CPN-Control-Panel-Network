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

## File Manager: Fix permissions

Root and Site File Manager expose a **Fix permissions** toolbar action that uses the
same selection checkmarks as Copy / Delete.

| Path class | Owner:group | Modes |
| --- | --- | --- |
| Under hosting home `/home/<fqdn>/...` | CPN website Unix account `user:user` (mailbox homes under `e-mails/<local>` prefer the mailbox Unix account when present) | dirs `0755`, files `0644` |
| Outside hosting homes | Keep existing non-root owner (modes only). Root-owned non-site paths are refused | dirs `0755`, files `0644` |

Folders are fixed recursively. Protected system paths, `/`, the hosting home root,
symlinks, and jail escapes are refused. The action does not create new Unix users;
heal site ownership first when no account is found.
