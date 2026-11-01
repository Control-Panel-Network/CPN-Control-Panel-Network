# Sub-users (ACL secondary logins)

Date: 01/11/2026

## Intent

Panel owners create **sub-users**: secondary accounts that log into the **owner's panel** with a subset of section capabilities (ACL-style). This is not Reseller Center (separate child hosting accounts with quota pools).

## Shipped in MVP

- Account fields: `role=subuser`, `parent_owner`
- UI: `/account/users/sub-users` (create, list, edit capabilities, delete)
- Capabilities map to existing **sidebar visibility** grants (hide non-granted sections; 403 via `sidebar_access_guard`)
- Website-related caps also add **site ACL** `all_owned_by=<owner>` so plugins/site tools work on the owner's domains
- Site create (websites / sub-domains / WordPress create-if-missing) uses `resource_owner_for()` so new sites belong to the parent owner
- Reseller promote/assign refuses sub-users
- Unit tests in `panel_subusers` and CSRF module

## Remaining work

1. **Email / DNS / Docker / FTP / Databases ownership filters**: many hubs still key off the signed-in username rather than `resource_owner_for()`. Audit and switch list/create filters for those areas when the sub-user has the matching capability.
2. **Billing / shop / API tokens**: decide whether sub-users may view billing; default deny. Gate `/account/api-access` and payment routes explicitly.
3. **Non-admin package owners**: today only the bootstrap panel admin (`is_panel_admin`) can create sub-users. Product may want site/package owners to create sub-users scoped to their own sites only.
4. **Audit trail**: log sub-user actions with `acting_as=<parent_owner>` in the panel action log.
5. **MFA policy**: confirm whether sub-users inherit owner MFA requirements or keep per-account TOTP/passkeys (current: normal per-account MFA flags from `create_account`).
6. **UI polish**: dedicated Modify Sub-user page, disable/enable without delete, capability presets (Email-only, Websites-only).
7. **Docs**: operator guide under Documentation hub; keep Reseller vs Sub-users distinction clear in Users hub copy.
8. **Backup/restore**: include `parent_owner` / sub-user role in account restore apply paths and document migration from older account JSON.

## Design note

Do **not** fold Sub-users into Reseller Center. Reseller = multi-tenant hierarchy with quotas. Sub-users = ACL helpers on one owner panel. Create ACL / Modify ACL remain the low-level grant editors; Sub-users is the productized create+grant flow.
