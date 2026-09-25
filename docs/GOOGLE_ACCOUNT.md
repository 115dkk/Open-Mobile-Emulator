# Google account on an uncertified guest (draft, R8)

The guest is not a Google-certified device. Google lets owners of such devices
register them once so that Play services will sign in. The product shows the
required ID and links to the registration page. It never registers on behalf of
the user and never alters device identity to pass certification.

## Procedure (to be validated in M0 step 8)

1. Boot the guest and finish the first-run wizard **without** adding a Google
   account.
2. Obtain the Google Services Framework (GSF) Android ID. Two ways:
   - With `adb root` available (Bliss debug builds): read
     `/data/data/com.google.android.gsf/databases/gservices.db`, table `main`,
     row `android_id`. The value is decimal; convert it to hexadecimal.
   - Without root: install a device-ID viewer app that displays the GSF ID.
3. Open https://www.google.com/android/uncertified/ while signed in to the
   Google account you intend to use, paste the GSF ID, submit.
4. Wait a few minutes, then add the account in guest Settings.
5. Record the outcome (sign-in works or not, Play Store state) in
   `docs/evidence/M0/`.

## What the product does and does not do

- Does: display the GSF ID, open the registration page, explain the wait.
- Does not: submit the form, change `ro.product.*` beyond `manifests/device-profile.prop`,
  spoof certification state, or promise that Play Store or in-app purchases work.
