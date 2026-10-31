# Personal data

This repository carries no personal data. No tracked file holds a mail address, a home path, a telephone
number, a token or a private network address, and no commit message does. A value of that kind a program needs
at run time is read from its environment or its settings, never written into the tree.

A person's name, handle and account id are personal data too. A tracked file holds none of them either; the
names check holds it to the names a list kept outside the checkout gives (`docs/checks.md`).

The receiver holds the same line at run time over what people say: a secret pasted into a room is taken out
of the words before anything keeps or forwards them, so the archive, the providers a room is carried to and every
turn see a marker where it stood (`docs/messages.md`, 「Secrets taken out」).

## What is prohibited

- **A mail address** at a real domain, in a test fixture, a document or a string alike.
- **A home path** under a person's home directory, on any platform.
- **A telephone number** in a national or international form.
- **A token** of any service: an access key, a personal access token, a session token or a private key.
- **A private network address** in a private, shared, link-local or unique-local range, IPv4 or IPv6.

A test fixture uses a neutral invented name. A document that has to show a credential's place writes a
placeholder as a name in angle brackets, never a credential-shaped value.

## What the check lets stand

- **Documentation domains.** The domains RFC 2606 and RFC 6761 reserve for documentation name no person and no
  machine, and an address there is accepted: `example.com`, `example.net`, `example.org`, and any name under
  `.example`, `.test`, `.invalid` or `.localhost`.
- **The loopback address.** It names the machine the program itself runs on, not a host on a private network.
  The service's default address is the loopback one, and it is accepted.

## The check

The suite carries one check over every tracked file, `personal_data` under `tests/`. It reads each file as text
and looks for the five kinds above. The kinds it reports are `mail`, `home-path`, `telephone`, `token` and
`private-network`.

A failure names the file, the line and the kind, and never the value that matched. A finding fails the suite; it
is no warning.

The check reads its own source as it reads any other tracked file, so a fixture that must contain a prohibited
value is assembled at run time instead of written out.
