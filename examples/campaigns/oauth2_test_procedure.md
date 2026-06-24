# Procédure de test OAuth2 — Terapi

Tests de validation des flows OAuth2 Client Credentials et Authorization Code
contre un serveur mock local (`mock-oauth2-server` de NAV IT).

---

## Prérequis

Docker installé et démarré.

```bash
docker run -d --name mock-oauth2 -p 8080:8080 ghcr.io/navikt/mock-oauth2-server:latest
```

Vérifier que le mock répond :

```bash
curl -s http://localhost:8080/default/.well-known/openid-configuration | python3 -m json.tool
```

Résultat attendu : JSON avec `token_endpoint`, `authorization_endpoint`, etc.

Valider le flow Client Credentials en ligne de commande avant de tester le TUI :

```bash
curl -s -X POST http://localhost:8080/default/token \
  -H "Content-Type: application/x-www-form-urlencoded" \
  -d "grant_type=client_credentials&client_id=terapi-test&client_secret=secret123&scope=api" \
  | python3 -m json.tool
```

Résultat attendu :
```json
{
    "token_type": "Bearer",
    "access_token": "eyJ...",
    "expires_in": 3599,
    "scope": "api"
}
```

---

## Endpoint de test pour les requêtes

Le mock expose un endpoint `userinfo` protégé pour valider que le Bearer token
est bien injecté dans les requêtes :

```
GET http://localhost:8080/default/userinfo
Authorization: Bearer <token>
```

Résultat attendu (avec un token valide) : JSON avec les claims du token (`sub`, `iss`, etc.)

---

## Test 1 — Client Credentials : flow nominal

**Setup :**
- Ouvrir Terapi : `cargo run`
- Aller sur l'onglet **Request**
- URL : `http://localhost:8080/default/userinfo`
- Méthode : `GET`
- Onglet **Auth** → `Space` pour cycler jusqu'à `OAuth2 Client Credentials`

**Remplir les champs** (↑/↓ pour naviguer, Enter pour éditer) :
| Champ | Valeur |
|-------|--------|
| Token URL | `http://localhost:8080/default/token` |
| Client ID | `terapi-test` |
| Client Secret | `secret123` |
| Scope | `api` |

**Action :** appuyer sur `s` (send)

**Résultats attendus :**
1. Banner jaune `⟳ fetching token…` s'affiche brièvement dans l'onglet Auth
2. Puis disparaît → ligne `● token cached` en vert
3. La requête part automatiquement après le fetch
4. Réponse HTTP 200 avec le JSON des claims du token
5. Status bar : `200 OK  XXms`

---

## Test 2 — Cache : pas de double fetch

**Action :** appuyer à nouveau sur `s` sans modifier les champs

**Résultats attendus :**
1. Aucun banner `⟳ fetching…` — le token en cache est réutilisé directement
2. Réponse HTTP 200 immédiate

**Validation :** dans les logs Docker, vérifier qu'un seul appel au token endpoint a eu lieu :
```bash
docker logs mock-oauth2 2>&1 | grep "token"
```

---

## Test 3 — Fetch manuel (`f`)

**Setup :** depuis l'onglet Auth, modifier temporairement le Client ID en `autre-client`
puis le remettre à `terapi-test` (le cache est invalidé car la clé change).

**Action :** appuyer sur `f` (fetch token sans envoyer la requête)

**Résultats attendus :**
1. Banner `⟳ fetching token…` apparaît
2. Disparaît → `● token cached` en vert
3. Aucune requête envoyée vers `/userinfo`

---

## Test 4 — Erreur token URL invalide

**Setup :** changer Token URL en `http://localhost:8080/wrong/token`

**Action :** `f`

**Résultats attendus :**
1. Banner rouge `✗ token endpoint returned HTTP 404: …`
2. Status bar affiche `OAuth2 error: …`
3. Appuyer sur `Esc` → banner disparaît, `oauth2_wait_state` revient à Idle

**Restaurer** Token URL à `http://localhost:8080/default/token` avant de continuer.

---

## Test 5 — Erreur réseau (mock arrêté)

```bash
docker stop mock-oauth2
```

**Action :** `f` dans le TUI

**Résultats attendus :**
1. Banner rouge `✗ request failed: …connection refused…`
2. `Esc` efface l'erreur

```bash
docker start mock-oauth2
```

---

## Test 6 — Persistance TOML

**Setup :** avec les champs OAuth2 Client Credentials remplis, sauvegarder la requête :
`S` → nommer `oauth2-cc-test` → choisir une collection

**Vérification :** ouvrir le fichier TOML de la collection dans `~/.config/terapi/collections/`
et vérifier la présence des champs :

```toml
[auth]
auth_type = "oauth2_client_credentials"
oauth2_token_url = "http://localhost:8080/default/token"
oauth2_client_id = "terapi-test"
oauth2_client_secret = "secret123"
oauth2_scope = "api"
```

**Résultat attendu :** tous les champs présents, token absent du fichier.

Recharger la requête depuis Collections (`e`) → les champs sont restaurés.

---

## Test 7 — Authorization Code : flow nominal

**Setup :**
- Même URL : `http://localhost:8080/default/userinfo`
- Onglet Auth → cycler jusqu'à `OAuth2 Authorization Code`

**Remplir les champs :**
| Champ | Valeur |
|-------|--------|
| Token URL | `http://localhost:8080/default/token` |
| Client ID | `terapi-test` |
| Client Secret | `secret123` |
| Scope | `openid` |
| Auth URL | `http://localhost:8080/default/authorize` |
| Redirect Port | `9876` |

**Action :** `f`

**Résultats attendus :**
1. Le navigateur s'ouvre sur `http://localhost:8080/default/authorize?client_id=…`
2. Banner jaune `⟳ waiting for browser callback on port 9876…` dans le TUI
3. Le mock redirige automatiquement vers `http://127.0.0.1:9876/?code=XXX`
4. Terapi capture le code, l'échange contre un token
5. Banner disparaît → `● token cached`

**Appuyer sur `s` :** requête part avec `Authorization: Bearer <token>`, réponse 200.

---

## Test 8 — Authorization Code : annulation

**Setup :** vider le cache (changer Client ID et le remettre)

**Action :** `f` → navigateur s'ouvre

**Action :** dans le TUI, appuyer sur `Esc` sans valider dans le navigateur

**Résultats attendus :**
1. Banner disparaît
2. Status bar : `OAuth2 fetch cancelled`
3. `request_loading` revient à false (TUI réactif)

---

## Test 9 — Port déjà occupé

```bash
python3 -c "import socket; s=socket.socket(); s.bind(('127.0.0.1',9876)); s.listen(1); input('press enter')" &
```

**Action :** `f` avec Authorization Code configuré sur port 9876

**Résultat attendu :** banner rouge `✗ cannot bind port 9876: …`

```bash
kill %1
```

---

## Nettoyage

```bash
docker stop mock-oauth2
docker rm mock-oauth2
```

---

## Matrice de résultats

| Test | Attendu | Obtenu | OK ? |
|------|---------|--------|------|
| 1 — CC nominal | 200 + token cached | 200 OK, `● token cached` vert | ✓ |
| 2 — Cache réutilisé | Pas de re-fetch | Requête directe, pas de banner fetch | ✓ |
| 3 — Fetch manuel `f` | Token sans send | Token mis en cache, pas de requête | ✓ |
| 4 — URL invalide / réseau coupé | Banner erreur connexion | `✗ request failed: … connection refused` | ✓ |
| 5 — Réseau coupé | Skippé (même cas que 4) | — | — |
| 6 — Persistance TOML | Champs dans .toml, token absent | Tous les champs présents, token absent | ✓ |
| 7 — Auth Code nominal | Navigateur + 200 | Navigateur ouvert, callback capturé, 200 OK | ✓ |
| 8 — Auth Code annulé | Esc annule proprement | `OAuth2 fetch cancelled`, TUI réactif | ✓ |
| 9 — Port occupé | Banner erreur bind | `✗ cannot bind port 9876: Address already in use` | ✓ |

## Bugs trouvés et corrigés pendant les tests

- **Race condition clé de cache** — token stocké sous la mauvaise clé si les champs sont modifiés pendant le fetch async. Fix : clé calculée avant `tokio::spawn`, transportée dans le canal avec le résultat.
- **CC et AC partageaient la même clé** — mêmes credentials mais types différents s'écrasaient mutuellement. Fix : clé inclut désormais `auth_type`.
- **OAuth2 CC/AC absents du sélecteur** — tableau hardcodé dans `render_auth_editor` ne contenait pas les deux variants. Fix : labels courts `OAuth2 CC` / `OAuth2 AC` ajoutés.
