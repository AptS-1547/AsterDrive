# Mail

::: tip This page covers the "Mail Delivery" runtime system settings group
Mail configuration is not in `config.toml`; it is all under `Admin -> System Settings -> Mail Delivery`.
If you plan to enable registration, password recovery, or email address changes, configure this group first: **set up mail before enabling registration**.
:::

Features that depend on mail:

- Email activation after public registration
- Password recovery on the login page
- Email address changes in `Settings -> Security`
- Test emails sent by administrators

Entry point:

```text
Admin -> System Settings -> Mail Delivery
```

## Recommended Order

1. Fill in SMTP server, port, and security mode
2. Fill in username and password if needed
3. Fill in sender address and sender name
4. **Send a test email to yourself first**
5. Then test registration activation, password reset, and email address changes

::: warning The cost of doing this in the wrong order
If you enable public registration first and configure mail later, a batch of user accounts may already have been created but cannot receive activation emails. They will all be stuck at "waiting for activation".
:::

## Options

| Option | Purpose |
| --- | --- |
| `mail_smtp_host` | SMTP server address |
| `mail_smtp_port` | SMTP port, default `587` |
| `mail_security` | Security mode. `465` usually means implicit SSL/TLS; other ports usually use STARTTLS. |
| `mail_smtp_username` | SMTP login username |
| `mail_smtp_password` | SMTP login password |
| `mail_from_address` | Sender email address shown to recipients |
| `mail_from_name` | Sender name shown to recipients |

::: tip Username and password handling

- SMTP does not require authentication - leave both empty
- SMTP requires authentication - **fill in both together** to avoid providing only one of them

:::

If you do not usually manage mail systems, think of SMTP simply as "the connection information for the server that sends mail".

## How to Confirm Mail Can Actually Be Sent

The `Mail Delivery` page has a `Send Test Email` button.

Common usage:

- Send directly to the current administrator email address
- Temporarily change it to another external email address to confirm non-internal domains can receive mail too

After the test passes, do two more things:

1. On the login page, try "register and receive activation email" or "forgot password" once
2. Confirm that `Admin -> System Settings -> Site Configuration -> Public Site URL` is correct

## What Can Mail Templates Change?

AsterDrive currently has 5 built-in template groups:

- Registration activation
- Email address change confirmation
- Password reset
- Password reset result notification
- Old email address change notification

Each group can change:

- Subject
- Email body (HTML)

::: tip Do not guess variable names yourself
The right side of the page lists the magic variables available to the current mail template. Fill them in from there.
:::

## Why Configure `Public Site URL` Together?

Activation links, password reset links, and email-change confirmation links all need to generate **addresses that can be opened externally**.

If the real access address is:

```text
https://drive.example.com
```

Set it here:

```text
Admin -> System Settings -> Site Configuration -> Public Site URL
```

If the same instance has multiple public entry points, enter them one by one in the list. Background flows such as email delivery do not have the current browser Host, so they use the first item as the default origin. Put the default origin you most want users to click first.

::: warning Enter only the site origin
Do not include a path. Do not include `/api`. Enter only the origin, such as `https://drive.example.com`.
:::

## What Happens When It Is Misconfigured

| Symptom | Likely Problem |
| --- | --- |
| New users can register but do not receive activation emails | SMTP cannot connect, or the recipient side rejects the email |
| The forgot-password button works, but no reset link appears in the mailbox | Same as above, or `Public Site URL` is missing |
| Users can start an email address change, but the new mailbox does not receive confirmation | Same as above |
| Test email fails | SMTP configuration is wrong, or outbound network access is blocked |

Troubleshooting checklist:

1. SMTP host, port, security mode, account, and password
2. Whether the SMTP service allows the sender address
3. Whether `Public Site URL` has been changed to the real external HTTP(S) origin; if there are multiple entry points, whether the mail default origin is first
4. Check both inbox and spam folders
5. After mail is restored, resend activation emails or email-change confirmation emails
