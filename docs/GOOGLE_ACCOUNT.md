# Google account on an uncertified guest (draft, R8)

The guest is not a Google-certified device. Google lets owners of such devices
register them once so that Play services will sign in. The product shows the
required ID and links to the registration page. It never registers on behalf of
the user and never alters device identity to pass certification.

## Procedure (validated in M0 step 8 on 2026-09-26)

1. Boot the guest and finish the first-run wizard **without** adding a Google
   account.
2. Obtain the Google Services Framework (GSF) Android ID. Two ways:
   - With `adb root` available (verified on Bliss 16.9.7, a userdebug build): read
     `/data/data/com.google.android.gsf/databases/gservices.db`, table `main`,
     row `android_id`, with the image's `/system/bin/sqlite3`. The value is
     decimal; convert it to hexadecimal. The ID exists only after the first
     Google check-in, which the setup wizard's "Getting your tablet ready" step
     performs. Pass the SQL through a pushed file (`sqlite3 db < /data/local/tmp/q.sql`)
     because quotes do not survive PowerShell, adb and the guest shell
     (`docs/evidence/M0/google-account.md`).
   - Without root: install a device-ID viewer app that displays the GSF ID.
3. Open https://www.google.com/android/uncertified/ while signed in to the
   Google account you intend to use, paste the GSF ID, submit. The page accepts
   the 16-digit hexadecimal form (it did on 2026-09-26); keep the decimal form
   at hand in case the form changes.
4. Wait about ten minutes, then add the account in guest Settings. The product
   can open the page for you: `am start -a android.settings.ADD_ACCOUNT_SETTINGS
   --esa account_types com.google` shows Google's sign-in directly. The
   credentials are typed inside the guest window, never through the product.
5. Result recorded in `docs/evidence/M0/google-account.md`: the account was
   added without a certification warning, Google Play Games fetched the player
   profile for it, a game that calls Play Games sign-in at start-up stopped
   opening the system "Add account" page, and the Play Store opened. In-app
   purchases were not tested.

## What the product does and does not do

- Does: display the GSF ID, open the registration page, explain the wait.
- Does not: submit the form, change `ro.product.*` beyond `manifests/device-profile.prop`,
  spoof certification state, or promise that Play Store or in-app purchases work.
