"""Fake signed-in accounts for the parity harness (`accounts: <fixture>` step).

Neither app can sign in during a run (no credentials, and the OAuth page
would open in the browser), so the signed-in surfaces — the blank slate's
repository list, Welcome › Configure Git's account options — get their
accounts injected instead:

- GHD: the `AppStore` is found through the React fiber of the root `App`
  component; its `accounts` and the `ApiRepositoriesStore` state are replaced
  and an update is emitted (no API call happens: every account already has its
  repositories, and the fake tokens are never used).
- Corvane: the `fake-accounts` control hook puts the same accounts and
  repositories into `AppState` (`crates/corvane/src/parity_control.rs`).
"""

import json

DOTCOM = "https://api.github.com"
GHES = "https://ghe.example.com/api/v3"


def _repo(endpoint: str, owner: str, name: str, private=False, fork=False, archived=False) -> dict:
    web = "https://github.com" if endpoint == DOTCOM else endpoint.split("/api/")[0]
    return {"owner": owner, "name": name, "private": private, "fork": fork, "archived": archived,
            "html_url": f"{web}/{owner}/{name}", "clone_url": f"{web}/{owner}/{name}.git"}


_OCTOCAT = {
    "endpoint": DOTCOM, "id": 583231, "login": "octocat", "name": "Mona Lisa Octocat",
    "emails": [
        {"email": "mona@example.com", "primary": True, "visibility": "public"},
        {"email": "583231+octocat@users.noreply.github.com", "primary": False, "visibility": None},
    ],
    "repositories": [
        _repo(DOTCOM, "octocat", "Hello-World"),
        _repo(DOTCOM, "octocat", "Spoon-Knife"),
        _repo(DOTCOM, "octocat", "linguist", fork=True),
        _repo(DOTCOM, "octocat", "octocat.github.io"),
        _repo(DOTCOM, "octocat", "secret-plans", private=True),
        _repo(DOTCOM, "octocat", "git-consortium", archived=True),
        _repo(DOTCOM, "octocat", "test-repo1"),
        _repo(DOTCOM, "github", "docs"),
        _repo(DOTCOM, "github", "gitignore"),
        _repo(DOTCOM, "desktop", "desktop"),
        _repo(DOTCOM, "desktop", "dugite", private=True),
        _repo(DOTCOM, "zed-industries", "zed"),
    ],
}

_ENTERPRISE = {
    "endpoint": GHES, "id": 42, "login": "mona", "name": "",
    "emails": [{"email": "mona@corp.example.com", "primary": True, "visibility": "private"}],
    "repositories": [
        _repo(GHES, "mona", "handbook"),
        _repo(GHES, "platform", "infrastructure", private=True),
    ],
}

FIXTURES = {
    "dotcom": [_OCTOCAT],
    "enterprise": [_ENTERPRISE],
    "two": [_OCTOCAT, _ENTERPRISE],
}


def ghd_js(name: str) -> str:
    """Renderer JS that signs GHD in with the fixture's accounts."""
    return """(() => {
  const fixture = %s;
  const root = document.getElementById('desktop-app-container');
  const key = Object.keys(root).find(k => k.startsWith('__reactContainer') || k === '_reactRootContainer');
  let fiber = key === '_reactRootContainer' ? root._reactRootContainer._internalRoot.current : root[key];
  const stack = [fiber];
  let store = null;
  while (stack.length && !store) {
    const f = stack.pop();
    if (!f) continue;
    if (f.stateNode && f.stateNode.props && f.stateNode.props.appStore) store = f.stateNode.props.appStore;
    if (f.sibling) stack.push(f.sibling);
    if (f.child) stack.push(f.child);
  }
  if (!store) throw new Error('AppStore not found');
  const hostOf = e => e === 'https://api.github.com' ? 'GitHub.com' : new URL(e).hostname;
  const state = new Map();
  const accounts = fixture.map(a => {
    const account = {
      login: a.login, endpoint: a.endpoint, token: 'parity', avatarURL: '', id: a.id, name: a.name,
      plan: 'free', features: [], emails: a.emails.map(e => ({...e, verified: true})),
      get friendlyEndpoint() { return hostOf(this.endpoint) },
      withToken() { return this },
    };
    const repositories = a.repositories.map(r => ({
      ...r, owner: {login: r.owner, id: 1, type: 'User'}, default_branch: 'main', pushed_at: '2026-01-01T00:00:00Z',
      has_issues: true, parent: undefined, permissions: {admin: true, push: true, pull: true},
    }));
    state.set(account, {repositories, loading: false});
    return account;
  });
  store.accounts = accounts;
  store.apiRepositoriesStore.accountState = state;
  store.emitUpdate();
  return accounts.length;
})()""" % json.dumps(FIXTURES[name])


def corvane_arg(name: str) -> str:
    """The `fake-accounts` hook argument: `{accounts, repositories}` as serde
    reads `corvane_models::Account` / `GitHubRepository`."""
    accounts, repositories = [], {}
    for a in FIXTURES[name]:
        primary = next((e for e in a["emails"] if e["primary"]), None)
        emails = [e["email"] for e in sorted(a["emails"], key=lambda e: not e["primary"])]
        accounts.append({"endpoint": a["endpoint"], "id": a["id"], "login": a["login"],
                         "name": a["name"] or None, "avatar_url": None, "emails": emails,
                         "private_primary_email": bool(primary and primary["visibility"] == "private"),
                         "scopes": ["repo", "user", "workflow"], "plan": "free"})
        repositories[a["endpoint"]] = [{**r, "endpoint": a["endpoint"]} for r in a["repositories"]]
    return json.dumps({"accounts": accounts, "repositories": repositories})
