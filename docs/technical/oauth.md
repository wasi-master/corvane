# GitHub sign-in

Corvane signs in to GitHub.com with an OAuth app (client id
`corvane_github::CLIENT_ID`, overridable at build time with
`CORVANE_GITHUB_CLIENT_ID`). GitHub Desktop uses the OAuth web application
flow with a bundled client secret (`docs/technical/oauth.md` in
desktop/desktop). Corvane starts with the **browser flow** when the build has
a client secret (`CORVANE_GITHUB_CLIENT_SECRET`, set by release builds, never
committed) and with the **device flow** otherwise, since GitHub refuses the
browser flow's token exchange without a secret even with PKCE
(`incorrect_client_credentials`). The other flow stays one link away. Flag
`307-sign-in-flow` (auto / device / browser) overrides the choice.

The secret is extractable from any shipped binary (native apps are public
clients, RFC 8252); it adds little, because the device flow already issues
tokens for the client ID alone. PKCE keeps an intercepted callback code
useless, and the registered redirect URIs keep codes off other servers.

## Device flow

`corvane_github::auth`: `POST /login/device/code` → the dialog shows the
user code and opens `verification_uri` → `POST /login/oauth/access_token`
polled at the interval GitHub asked for → `GET /user` → token into the
macOS Keychain (`corvane_platform::keychain`). The OAuth app must have
"Enable Device Flow" ticked.

## Browser flow

`corvane_github::auth::WebFlow` + `corvane_core::web_flow`
(`Dispatcher::sign_in_web_flow` / `complete_web_flow`), reachable from the
sign-in dialog's primary button when a secret is built in, else its "Use
the browser flow instead" link:

1. A fresh CSRF `state` and a PKCE `code_verifier` (RFC 7636, S256) are
   generated; the browser opens
   `https://github.com/login/oauth/authorize?client_id&scope&state&redirect_uri&code_challenge&code_challenge_method=S256`.
2. GitHub redirects to the callback. Two callbacks exist:
   - `x-corvane-auth://oauth?code=…&state=…` — the URL scheme registered in
     `packaging/Info.plist`; macOS hands it to the running app
     (`Dispatcher::handle_app_url`). Used when Corvane runs from its bundle.
   - `http://127.0.0.1:<port>/callback?code=…&state=…` — a one-shot loopback
     listener (`LoopbackListener`) on an ephemeral port. Used for a bare
     binary (`cargo run`) and with `CORVANE_OAUTH_LOOPBACK=1`.
   Both must be listed as callback URLs of the OAuth app (GitHub allows
   any port for loopback addresses).
3. The `state` must match the flow that opened the browser (a reloaded
   callback page fails with "did not match this sign-in attempt").
4. `POST /login/oauth/access_token` with `client_id`, `code`,
   `redirect_uri`, `code_verifier` and, when the build has one, the
   `client_secret` from `CORVANE_GITHUB_CLIENT_SECRET`. Without a secret
   GitHub answers `incorrect_client_credentials` and the dialog says so —
   the device flow remains the way in.
5. The token goes through the same `finish_sign_in` as the device flow:
   `GET /user`, Keychain, account list. It never touches the settings store
   or the logs.

GitHub Enterprise: personal access tokens only (the OAuth app is registered
on GitHub.com).

## Testing without an account

`CORVANE_OAUTH_LOOPBACK=1` forces the loopback callback. The unit tests in
`corvane_github::auth::web_flow_tests` cover the PKCE challenge (RFC 7636
appendix B), the authorize URL, callback parsing and the loopback listener;
no GitHub account exists on the development machine, so the exchange
itself is unverified live.
