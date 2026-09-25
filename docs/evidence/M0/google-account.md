# M0 step 8: Google account path (partial, 2026-09-25)

Guest `default`, first boot after installation, no account added.

## What the wizard showed

- `CompatCheckinAndEarlyUpdate` ("Getting your tablet ready") completed on its
  own through slirp networking; logcat later shows
  `Checkin : [CheckinOperation] Checkin Operation finished with result: SUCCESS`.
- The Google sign-in page (`MinuteMaidActivity`) loaded normally on this
  uncertified guest (`first-boot-03-setup-wizard-google-sign-in.png`). It was
  skipped, as the procedure requires.

## GSF Android ID (read-only, for the user's own registration)

Read with `adb root` (works on this userdebug build) and the image's `sqlite3`:

```
adb -s 127.0.0.1:5555 root
adb -s 127.0.0.1:5555 push q.sql /data/local/tmp/q.sql      # select name,value from main where name='android_id';
adb -s 127.0.0.1:5555 shell 'sqlite3 /data/data/com.google.android.gsf/databases/gservices.db < /data/local/tmp/q.sql'
android_id|4121739…(가려짐)
```

| Form | Value |
|---|---|
| decimal (as stored) | `4121739…(가려짐)` |
| hexadecimal (what https://www.google.com/android/uncertified/ asks for) | `39335c0a…(가려짐)` |

The ID belongs to this disposable guest image only. Registration at the Google
page, waiting, and adding the account are user actions (R8) and were not done.
After the user registers, the remaining check is: add the account in guest
Settings, then confirm Play services sign-in state; record the result here.

Quoting note for the runbook: `adb shell sqlite3 db "select ..."` loses its
quotes between PowerShell, adb and the guest shell; redirecting a pushed `.sql`
file into `sqlite3` avoids the problem.
