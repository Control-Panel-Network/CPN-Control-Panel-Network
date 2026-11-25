# Tachyon notification `%count%` placeholder

## Symptom

Browser push from `panel.newstargeted.com` / Tachyon showed:

- Title: mailbox address (for example `info@newstargeted.com`)
- Body: `Du har %count% nye meldinger` (literal `%count%`)

## Root cause

Tachyon `app.js` calls:

```js
i18n('MESSAGE_LIST/NEW_MESSAGE_NOTIFICATION', { COUNT: len })
```

The i18n helper replaces `'%' + key + '%'`, so only `%COUNT%` is substituted.

English and almost all locales already use `%COUNT%`. Upstream Norwegian `nb/user.json` used lowercase `%count%`, so the toast kept the placeholder.

## Fix

1. Live: rewrite `NEW_MESSAGE_NOTIFICATION` in installed `nb/user.json` to `%COUNT%` (NewsTargeted + Three Brick applied 2026-11-21).
2. CPN: `install_snappymail_i18n::ensure_notification_count_placeholders()` on Tachyon/SnappyMail install/runtime and webmail heal.
3. Upstream follow-up: correct `kimusan/Tachyon` `localization/nb/user.json` in the next Tachyon release tarball so fresh extracts are clean before heal runs.

## Verify

```text
cargo test --lib install_snappymail_i18n
rg 'NEW_MESSAGE_NOTIFICATION' /opt/cpn-webmail/tachyon/**/localization/nb/user.json
```

Expect: `Du har %COUNT% nye meldinger`
