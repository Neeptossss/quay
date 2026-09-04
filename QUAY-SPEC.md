# Quay — spécification de démarrage

> Client desktop GitHub, local-first, clavier-first.
> Document destiné à un agent de développement. Nom de code provisoire : **Quay**. Renommer librement.

---

## 0. Comment utiliser ce document

Ce document est la source de vérité pour la phase M0 et M1. Il ne décrit pas un produit fini, il décrit un chemin de dérisquage.

Règles pour l'agent :

1. **N'écris pas d'UI avant que le M0 soit vert.** Le M0 est un spike en ligne de commande. Si les mesures du M0 invalident une hypothèse, remonte l'information et arrête-toi. Ne contourne pas.
2. **Chaque budget de performance de la section 4 est un test automatisé**, pas une intention. Un budget non testé est un budget non tenu.
3. **Quand une décision n'est pas tranchée ici, elle est dans la section 12.** Ne tranche pas seul, demande.
4. Tout code Rust est `#![deny(clippy::unwrap_used)]` dans les crates de sync et de stockage. Un `unwrap` sur une réponse réseau est un crash chez l'utilisateur.

---

## 1. Thèse produit

Le travail quotidien d'un développeur sur GitHub se répartit en deux moitiés :

- **Côté auteur** : fabriquer ses commits, découper, empiler, publier. Ce terrain est occupé (GitButler, Sublime Merge, Fork, jj).
- **Côté lecteur** : traiter la file de reviews, suivre l'état de ses PR, lire les tickets, vérifier la CI. Ce terrain est occupé par github.com, c'est-à-dire par une application web lente, orientée souris, mono-repo, sans état local.

**Quay attaque la seconde moitié.** L'objectif mesurable : un développeur traite dix pull requests en vingt minutes, sans souris, sur douze repos différents, sans jamais attendre un chargement.

Le produit est un client, pas un forge. Il ne remplace ni la PR GitHub ni le modèle de review de GitHub. Il rend l'interaction avec eux instantanée.

### Test d'acceptation de la thèse

Si un utilisateur retourne sur github.com pendant sa session de review pour une raison autre que « une fonctionnalité de niche pas encore implémentée », le produit a échoué. La cause la plus probable d'échec sera la latence perçue, pas l'absence de fonctionnalité.

---

## 2. Non-objectifs

Explicitement hors périmètre, et à défendre contre la dérive :

| Hors périmètre | Pourquoi |
|---|---|
| Parité fonctionnelle avec github.com | Cimetière de projets. 80 % en 3 mois, les 20 % restants en 3 ans. |
| Éditeur de code intégré | L'utilisateur a déjà un éditeur. On ouvre le sien. |
| Système de review propriétaire | On améliore la PR GitHub, on ne la remplace pas. Sinon toute l'équipe doit adopter. |
| Branches virtuelles / réécriture d'historique assistée | GitButler le fait mieux. On interopère. |
| Support GitLab / Bitbucket au M1 | L'abstraction multi-forge coûte cher et fige de mauvaises décisions tôt. On l'anticipe dans les types, on ne l'implémente pas. |
| Fonctionnalités IA | Pas avant que la boucle de base soit rapide. Une IA lente sur une app lente ne sauve rien. |
| Graphe de commits DAG complet | M3 au plus tôt. Ce n'est pas le point de douleur qu'on attaque. |

---

## 3. Contraintes dures de la plateforme GitHub

Ces contraintes ne sont pas négociables et dictent l'architecture. À relire avant toute décision de sync.

### 3.1 Pas de push serveur, et deux portes seulement vers le temps réel

GitHub n'expose pas de subscriptions GraphQL. Il n'existe que deux façons d'obtenir une propagation sous la seconde, et **aucune troisième** :

1. **Webhooks.** Nécessitent un accès admin sur le dépôt ou owner sur l'organisation. Un membre ordinaire ne peut pas en créer. Cela réintroduit le frein administrateur que §3.6 élimine par ailleurs.
2. **Un serveur qui polle avec le token de l'utilisateur.** Impose de stocker les tokens sur le serveur. Une compromission du serveur devient une compromission des dépôts privés de tous les utilisateurs. **Écarté définitivement.**

**Décision : le client polle, et la latence de propagation des changements des autres est bornée par le tier de fraîcheur (§7.1).** Ce n'est pas un pis-aller. Le plancher de 60 s ne s'applique qu'à la découverte d'un changement sur une entité que l'utilisateur ne regarde pas. L'entité affichée à l'écran est en tier hot, rafraîchie toutes les 10 s. Combinée aux mutations optimistes (§7.3) et au préchargement prédictif (§7.6), la latence perçue est celle d'une application locale.

Un canal push reste possible plus tard, en option, pour les équipes qui installent volontairement l'App (§5.5). L'architecture doit le permettre sans réécriture, pas le supposer.

### 3.2 Rate limits primaires

- 5 000 requêtes/heure en authentifié (REST), 5 000 points/heure en GraphQL.
- **Ce quota est partagé entre tous les PAT de l'utilisateur et toutes les apps OAuth autorisées en son nom.** On partage donc le budget avec `gh`, Dependabot local, les extensions VS Code de l'utilisateur, etc. Ne jamais consommer plus de ~30 % du quota en régime normal.
- Une requête conditionnelle (`If-None-Match` / `If-Modified-Since`) qui renvoie **304 ne décompte pas** du rate limit primaire, **à condition d'être authentifiée** avec un header `Authorization`. C'est le pilier de notre stratégie de polling. Non authentifié, le 304 coûte quand même une requête.

### 3.3 Rate limits secondaires — le vrai plafond

C'est ce qui tue les intégrations naïves :

- **100 requêtes concurrentes maximum**, limite partagée REST + GraphQL. → On plafonne à **8 requêtes concurrentes**, pas 100.
- **900 points/minute** sur un endpoint REST, **2 000 points/minute** en GraphQL. Barème : GET REST = 1 point, mutation REST = 5 points, requête GraphQL = 1 point, mutation GraphQL = 5 points.
- **90 secondes de temps CPU par 60 secondes réelles**, dont 60 s max pour GraphQL. Approximable par le temps de réponse cumulé. → Une requête GraphQL trop gourmande (PR avec 300 fichiers et 200 threads en une passe) consomme ce budget très vite. **Découper les requêtes GraphQL profondes.**
- 80 requêtes créatrices de contenu par minute, 500 par heure. Concerne nos mutations (commentaires, reviews).
- Un dépassement renvoie 403 ou 429, potentiellement avec `Retry-After`.

### 3.4 Notifications : le canal privilégié, avec un piège

L'endpoint `/notifications` est optimisé pour le polling : `Last-Modified` + `If-Modified-Since`, réponse 304 sans coût sur le rate limit, et un header **`X-Poll-Interval`** (60 s par défaut) qui indique la fréquence autorisée. **Respecter ce header, il peut augmenter en cas de charge.**

⚠️ **Piège bloquant à vérifier en M0** : la documentation indique que ces endpoints ne supportent que l'authentification par **personal access token classique**. Si c'est exact, cela exclut l'architecture GitHub App *et* les PAT fine-grained pour l'inbox, et impose le PAT classique ou l'OAuth app. C'est la première chose à mesurer empiriquement (tâche M0-1).

### 3.5 Ancrage des commentaires de review

Un commentaire de review est accroché à un `(path, diff_hunk, position | line+side+start_line)`. Après un force-push, il devient `outdated` et sa position n'a plus de sens dans le nouveau diff. Il faut :

- stocker à la fois `original_line` / `original_position` et `line` / `position`,
- afficher les threads outdated dans un rail séparé, pas les faire disparaître,
- ne jamais tenter de recalculer soi-même une position ; utiliser celle renvoyée par l'API.

Les opérations `resolveReviewThread` / `unresolveReviewThread` sont **GraphQL uniquement**. Prévoir un client hybride REST + GraphQL dès le départ.

### 3.6 Modes d'authentification — décision arrêtée

Le critère dominant n'est pas la sécurité ni l'élégance, c'est **le nombre de personnes à impliquer pour qu'un utilisateur puisse démarrer**. Toute solution qui nécessite l'intervention d'un administrateur d'organisation est disqualifiée.

| Mode | Friction d'onboarding | Étendue des droits | Verdict |
|---|---|---|---|
| **GitHub App** | Installation sur l'org par un **owner** | Intersection des permissions de l'app **et** de l'utilisateur, donc strictement plus limité | ❌ Éliminé |
| **OAuth App** | Restrictions d'accès **activées par défaut** sur toute nouvelle org → l'utilisateur doit **demander l'approbation d'un owner** | Droits réels de l'utilisateur via `repo` | ⚠️ Mode secondaire uniquement |
| **PAT classique** | Aucune. Seuls les tokens **fine-grained** sont soumis à approbation ; sauf restriction explicite de l'org, un PAT classique accède aux ressources de l'org **sans approbation préalable**, et l'autorisation par défaut est active | Droits réels de l'utilisateur | ✅ **Mode par défaut** |

Le PAT classique est également le seul mode que la documentation annonce comme supporté par `/notifications` (cf. §3.4), ce qui fait converger les deux contraintes.

**Décision : PAT classique en mode principal, OAuth device flow en mode secondaire optionnel.**

#### Scopes demandés

| Scope | Débloque | M1 |
|---|---|---|
| `repo` | PR, reviews, merge, issues, statuts sur les repos privés | requis |
| `notifications` | Inbox + polling conditionnel gratuit | requis |
| `read:org` | Équipes, membres, reviewers d'équipe | requis |
| `read:user` | Identité, résolution de `@me` | requis |
| `workflow` | Relance de jobs Actions, édition de workflows | optionnel |

#### Flux d'onboarding (PAT)

Ne jamais écrire « allez créer un token ». Le flux, en trois écrans maximum :

1. Écran d'accueil, un seul bouton. Il ouvre le navigateur sur une URL **pré-remplie** :
   ```
   https://github.com/settings/tokens/new
     ?description=Quay%20—%20client%20desktop
     &scopes=repo,read:org,notifications,read:user
   ```
   L'utilisateur choisit une expiration, clique Generate, copie.
2. L'app surveille le presse-papier au retour au premier plan ; si elle y détecte un préfixe de token GitHub, elle le pré-remplit. L'utilisateur valide, il ne colle même pas.
3. Validation immédiate : appel `/user` + `/user/orgs`, affichage du login et des orgs visibles. **Ne jamais accepter un token sans l'avoir validé.**

Le token part directement au keychain. Il ne transite jamais par l'IPC vers le frontend, il n'est jamais journalisé, il n'est jamais écrit sur disque en clair.

#### Verrous de développement

Le rate governor implémente deux garde-fous, actifs dès la première requête sortante :

- **Mode lecture seule** (`QUAY_READONLY=1`, par défaut en développement) : toute méthode HTTP non sûre est rejetée avant l'envoi.
- **Allowlist d'écriture** (`QUAY_WRITE_ALLOWLIST`) : aucune écriture vers un dépôt absent de la liste.

Ils existent parce qu'une session de test qui poste un commentaire ou merge sur une vraie pull request d'équipe est irréversible.

#### SAML SSO

Sur une org sous SSO, le token doit être **autorisé explicitement pour cette org**. C'est une action **de l'utilisateur lui-même**, pas de l'admin. Mais si elle n'est pas faite, les repos de l'org sont simplement absents, sans erreur.

Détection obligatoire : comparer les orgs retournées par `/user/orgs` avec celles réellement accessibles. En cas d'écart, afficher une bannière nommant l'org concernée avec un lien direct vers la page d'autorisation du token. **Jamais une liste vide sans explication.**

#### Le mur infranchissable

Un propriétaire d'entreprise peut restreindre l'accès par PAT pour toutes ses organisations, et les organisations **ne peuvent pas outrepasser** ce réglage. Si les OAuth apps sont également bloquées, aucun client tiers ne peut fonctionner. Ce n'est pas un bug à contourner. Détecter le cas, l'expliquer en une phrase claire, proposer d'exporter un message que l'utilisateur peut transmettre à son administrateur, et s'arrêter là.

### 3.7 Modèle de capacités et dégradation de l'interface

Puisque deux modes d'authentification coexistent, **une fonctionnalité indisponible ne doit jamais être découvrable puis échouer**. Un 403 en réponse à un clic est un bug d'interface, pas une erreur d'API.

#### Principe

Le core expose une structure de capacités, calculée une fois à la connexion, persistée, et rafraîchie au changement de token :

```rust
pub struct Capabilities {
    pub auth_mode: AuthMode,              // PatClassic | OAuth
    pub notifications: Support,
    pub review_submit: Support,
    pub thread_resolve: Support,
    pub merge: Support,
    pub actions_rerun: Support,
    pub org_teams: Support,
    // ...
}

pub enum Support {
    Available,
    Unavailable { reason: Reason },       // affiché tel quel à l'utilisateur
    Unknown,                              // pas encore sondé
}

pub enum Reason {
    ScopeMissing(&'static str),           // action corrective : ajouter un scope
    AuthModeUnsupported,                  // action corrective : passer en PAT
    OrgPolicy(String),                    // action corrective : voir l'admin
    SsoUnauthorized(String),              // action corrective : autoriser le token
}
```

#### Comment les capacités sont déterminées

**Ne jamais coder en dur « OAuth ne sait pas faire X ».** Les règles de GitHub changent sans préavis et une table figée dans le binaire devient fausse. Trois sources, dans cet ordre :

1. **Les scopes réellement accordés**, lus dans le header `X-OAuth-Scopes` de la première réponse. Source de vérité immédiate et gratuite.
2. **Un sondage actif au premier lancement** : une requête en lecture seule sur chaque capacité douteuse (typiquement `/notifications`). Résultat mis en cache, réévalué au changement de token ou toutes les 24 h.
3. **L'apprentissage par l'échec** : un 403 ou 404 sur une capacité marquée `Available` la fait basculer en `Unavailable` avec la raison extraite de la réponse, et remonte un événement à l'UI. L'app ne se retrouve jamais dans un état où elle propose en boucle une action qui échoue.

#### Règles d'affichage

| Situation | Comportement |
|---|---|
| Capacité indisponible **structurellement** (mode d'auth, politique d'org) | La fonctionnalité **n'apparaît pas**. Absente de la palette de commandes, son raccourci n'est pas lié, aucune entrée fantôme. |
| Capacité indisponible mais **récupérable par l'utilisateur** (scope manquant, SSO non autorisé) | Visible mais **désactivée**, avec l'action corrective à un clic. Exemple : « Relancer le job — nécessite le scope `workflow`. Mettre à jour le token. » |
| Capacité `Unknown` | Traitée comme disponible, avec bascule automatique en cas d'échec. On ne bloque pas par précaution. |

Trois interdits :

- Aucune entrée grisée sans explication de la raison **et** de l'action corrective. Un bouton gris muet est pire qu'un bouton absent.
- Aucune bascule d'état pendant que l'utilisateur interagit avec la zone concernée. On attend la fin de l'interaction.
- Le mode dégradé doit être **visible en permanence**, pas seulement au moment de l'échec : un indicateur discret dans la barre d'état, avec le détail au clic.

Test d'acceptation : lancer l'app avec un token à scopes volontairement réduits. Le parcours doit rester cohérent de bout en bout, sans une seule erreur 403 remontée à l'utilisateur.

### 3.8 Entreprise

- GitHub Enterprise Server : URL de base configurable dès le M1 (`https://HOST/api/v3`). Coût faible si prévu, coût élevé si rétrofit.
- Les politiques d'organisation (restriction des PAT, restriction des OAuth apps) sont détectées et expliquées, jamais contournées.

---

## 4. Budgets de performance

**Ce sont des tests, pas des vœux.** Chaque ligne doit avoir un bench automatisé qui casse la CI en cas de régression. Mesures sur une machine de référence (M-series 8 Go / équivalent x86 2020) avec un dataset de 20 repos, 300 PR ouvertes, 5 000 commentaires en cache.

| Métrique | Budget | Comment mesurer |
|---|---|---|
| Démarrage à froid → première liste peinte | **< 400 ms** | Trace Tauri, timestamp process start → paint |
| Frappe clavier → pixel mis à jour | **< 16 ms (p99)** | Instrumentation frontend, `performance.now()` |
| Navigation PR suivante (données en cache) | **< 50 ms** | Idem |
| Requête SQLite de liste (inbox filtrée) | **< 5 ms (p99)** | Bench Rust (`criterion`) |
| Ouverture d'un diff de 2 000 lignes | **< 120 ms** | Bench, rendu virtualisé |
| RAM au repos, 20 repos synchronisés | **< 250 Mo RSS** | Mesure process |
| CPU au repos, fenêtre en arrière-plan | **< 0,5 %** | Le polling ne doit pas réveiller le CPU inutilement |
| Consommation du quota GitHub en régime normal | **< 1 500 req/h** | Compteur interne, exposé dans l'app |
| Navigations servies depuis le cache en < 50 ms | **> 85 %** | `preload_outcome`, fenêtre glissante |

Règle absolue : **aucune interaction utilisateur ne déclenche une requête réseau bloquante.** Toute lecture vient de SQLite. Toute écriture est optimiste.

---

## 5. Architecture

### 5.1 Vue d'ensemble

```
┌─────────────────────────────────────────────────────┐
│  Frontend (WebView)                                 │
│  Svelte 5 + TypeScript                              │
│  - store en mémoire, hydraté depuis le core         │
│  - virtualisation des listes et des diffs           │
│  - zéro logique métier, zéro appel réseau           │
└───────────────▲─────────────────────┬───────────────┘
                │ events (push)       │ commands (IPC)
┌───────────────┴─────────────────────▼───────────────┐
│  Core Rust (Tauri 2)                                │
│                                                     │
│  ┌───────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ query     │  │ mutation     │  │ scheduler    │  │
│  │ layer     │  │ queue        │  │ (polling)    │  │
│  └─────┬─────┘  └──────┬───────┘  └──────┬───────┘  │
│        │               │                 │          │
│  ┌─────▼───────────────▼─────────────────▼───────┐  │
│  │ store : SQLite (WAL) + cache ETag             │  │
│  └───────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────┐  │
│  │ forge client : REST + GraphQL, rate governor  │  │
│  └───────────────────────────────────────────────┘  │
│  ┌───────────────────────────────────────────────┐  │
│  │ git local : gitoxide (M2+)                    │  │
│  └───────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────┘
```

### 5.2 Découpage en crates

```
crates/
  quay-core/        types du domaine, sans I/O, testable en isolation
  quay-store/       SQLite, migrations, requêtes, cache ETag
  quay-forge/       client GitHub REST+GraphQL, rate governor, auth
  quay-sync/        scheduler, réconciliation, file de mutations
  quay-git/         wrapper gitoxide (M2)
  quay-app/         binaire Tauri, commands IPC, événements
apps/
  desktop/          frontend Svelte
```

**Contrainte de dépendances** : `quay-core` ne dépend de rien. `quay-store` et `quay-forge` ne se connaissent pas. `quay-sync` orchestre les deux. Un test d'architecture en CI vérifie le graphe de dépendances.

### 5.3 Choix techniques et justifications

| Choix | Raison | Alternative rejetée |
|---|---|---|
| Tauri 2 | WebView système, binaire léger, cœur Rust partagé desktop/CLI | Electron : ~150 Mo de RAM de base sur une app qu'on laisse ouverte 8 h |
| Svelte 5 (runes) | Pas de VDOM, réactivité granulaire, sortie compacte. La différence est visible sur une liste de 500 lignes qui se met à jour à chaque frappe | React : coût de réconciliation incompatible avec le budget de 16 ms |
| SQLite en WAL | Lectures concurrentes pendant écriture, transactionnel, fiable | JSON sur disque : pas de requêtes, pas d'atomicité |
| `reqwest` + `hyper` | Contrôle fin des headers conditionnels | Octocrab : masque les ETags dont on a absolument besoin. Réévaluable. |
| gitoxide plutôt que git2 | Rust pur, pas de FFI libgit2, meilleures perfs sur le revwalk | git2-rs : acceptable en fallback pour les opérations non couvertes |
| Tokens dans le keychain OS | Ne jamais écrire un token en clair sur disque | Fichier de config : inacceptable |

### 5.4 Frontière IPC

Le frontend n'appelle **jamais** le réseau. Deux canaux seulement :

- **Commands** (frontend → core) : `list_inbox(filter)`, `get_pr(id)`, `submit_review(...)`. Toutes retournent depuis SQLite, en moins de 5 ms.
- **Events** (core → frontend) : `inbox_changed`, `pr_updated { id }`, `sync_state { .. }`, `mutation_failed { .. }`. Le frontend s'abonne et invalide son store.

Le payload IPC est un coût réel. Ne jamais renvoyer une liste complète quand un delta suffit.

### 5.5 Backend — hors M1, mais anticipé

Le backend n'est **jamais une dépendance**. Le client fonctionne intégralement sans lui. Si le serveur est injoignable, l'application ne le signale même pas : elle continue en polling. Cette règle est testée, pas espérée.

#### Ce que le backend fait, par ordre de valeur

| Service | Token utilisateur requis ? | Contenu de dépôt stocké ? | Milestone |
|---|---|---|---|
| Synchronisation de configuration (vues, raccourcis, layouts), chiffrée côté client | non | non | M2 |
| Priors de prédiction agrégés et anonymisés (§7.6, couche 3) | non | non | M3 |
| Vues et conventions partagées d'équipe | non | non | M3 |
| Mises à jour, licences, télémétrie opt-in | non | non | M2 |
| Canal de signal push, si une équipe installe l'App | non | non | M3, optionnel |

#### Invariant de sécurité

**Aucun token utilisateur ne transite ni ne réside sur le backend. Aucun contenu de dépôt non plus.** Cet invariant est ce qui rend le produit défendable auprès d'un owner d'organisation, et ce qui réduit l'exposition en cas de compromission à des données sans valeur offensive.

La synchronisation de configuration est chiffrée côté client : le serveur ne stocke que des octets opaques dont il n'a pas la clé.

#### Le canal de signal, si un jour il existe

```
GitHub ──webhook──> backend ──{repo_id, pr_number, updated_at}──> client
                                                                    │
client ──fetch avec SON PROPRE token──> GitHub ─────────────────────┘
```

Le backend ne transmet qu'un signal de changement, jamais de données. Le client va chercher le contenu lui-même, avec ses propres permissions : le modèle d'autorisation de GitHub s'applique sans être réimplémenté.

Identité du client auprès du backend : OAuth device flow avec le seul scope `read:user`. Autorisation d'abonnement à un dépôt : vérifiée par le backend via **sa propre installation d'App**, jamais via le token de l'utilisateur.

Sans App installée, le backend n'apporte rien de plus que le client seul en matière de fraîcheur. C'est assumé.

---

## 6. Schéma de données

Principes : chaque entité distante porte sa fraîcheur et son ETag. Le contenu brut de l'API est conservé pour permettre une remigration sans re-fetch.

```sql
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

-- comptes et forges
CREATE TABLE account (
  id            INTEGER PRIMARY KEY,
  host          TEXT NOT NULL,          -- api.github.com ou GHES
  login         TEXT NOT NULL,
  auth_kind     TEXT NOT NULL,          -- pat_classic | oauth | app
  keychain_ref  TEXT NOT NULL,          -- jamais le token lui-même
  UNIQUE(host, login)
);

CREATE TABLE repo (
  id            INTEGER PRIMARY KEY,
  account_id    INTEGER NOT NULL REFERENCES account(id),
  node_id       TEXT NOT NULL UNIQUE,
  owner         TEXT NOT NULL,
  name          TEXT NOT NULL,
  default_branch TEXT,
  local_path    TEXT,                   -- si un clone local est associé
  is_tracked    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE pull_request (
  id            INTEGER PRIMARY KEY,
  repo_id       INTEGER NOT NULL REFERENCES repo(id),
  number        INTEGER NOT NULL,
  node_id       TEXT NOT NULL UNIQUE,
  title         TEXT NOT NULL,
  state         TEXT NOT NULL,          -- open | closed | merged
  is_draft      INTEGER NOT NULL,
  author        TEXT NOT NULL,
  base_ref      TEXT NOT NULL,
  head_sha      TEXT NOT NULL,
  review_state  TEXT,                   -- approved | changes_requested | ...
  checks_state  TEXT,                   -- success | failure | pending | none
  mergeable     TEXT,
  updated_at    TEXT NOT NULL,
  raw           BLOB,                   -- payload API compressé
  UNIQUE(repo_id, number)
);

CREATE INDEX idx_pr_inbox ON pull_request(state, updated_at DESC);

CREATE TABLE review_thread (
  id            INTEGER PRIMARY KEY,
  pr_id         INTEGER NOT NULL REFERENCES pull_request(id) ON DELETE CASCADE,
  node_id       TEXT NOT NULL UNIQUE,
  path          TEXT NOT NULL,
  line          INTEGER,                -- position actuelle, NULL si outdated
  side          TEXT,
  original_line INTEGER,                -- position d'origine, toujours conservée
  diff_hunk     TEXT,
  is_resolved   INTEGER NOT NULL DEFAULT 0,
  is_outdated   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE review_comment (
  id            INTEGER PRIMARY KEY,
  thread_id     INTEGER NOT NULL REFERENCES review_thread(id) ON DELETE CASCADE,
  node_id       TEXT NOT NULL UNIQUE,
  author        TEXT NOT NULL,
  body          TEXT NOT NULL,
  created_at    TEXT NOT NULL
);

-- fraîcheur et cache conditionnel
CREATE TABLE resource_cache (
  key           TEXT PRIMARY KEY,       -- ex: "pr:MDExOlB1..."
  etag          TEXT,
  last_modified TEXT,
  fetched_at    INTEGER NOT NULL,
  stale_after   INTEGER NOT NULL,       -- timestamp, pilote le scheduler
  tier          TEXT NOT NULL           -- hot | warm | cold
);

-- file de mutations optimistes
CREATE TABLE mutation (
  id            INTEGER PRIMARY KEY,
  kind          TEXT NOT NULL,          -- approve_pr | post_comment | resolve_thread | ...
  target        TEXT NOT NULL,          -- node_id concerné
  payload       TEXT NOT NULL,          -- JSON
  idempotency   TEXT NOT NULL UNIQUE,   -- évite le double envoi après crash
  state         TEXT NOT NULL,          -- pending | inflight | failed | done
  attempts      INTEGER NOT NULL DEFAULT 0,
  last_error    TEXT,
  created_at    INTEGER NOT NULL
);

-- vues sauvegardées : la primitive qui remplace les "templates métier"
CREATE TABLE saved_view (
  id            INTEGER PRIMARY KEY,
  name          TEXT NOT NULL,
  query         TEXT NOT NULL,          -- DSL, cf. §8.4
  columns       TEXT NOT NULL,          -- JSON
  sort          TEXT NOT NULL,
  position      INTEGER NOT NULL,
  shortcut      TEXT                    -- ex: "g r"
);

-- collecte pour le modèle prédictif (§7.6)
-- À créer dès le M1 même si rien ne la lit encore : sans historique,
-- la couche 2 repart de zéro le jour où on l'implémente.
CREATE TABLE navigation_event (
  id            INTEGER PRIMARY KEY,
  from_state    TEXT NOT NULL,          -- ex: "inbox:list"
  to_state      TEXT NOT NULL,          -- ex: "pr:detail"
  action        TEXT NOT NULL,          -- ex: "key:j" | "cmd:pr.open"
  entity_kind   TEXT,                   -- pr | issue | repo | NULL
  dwell_ms      INTEGER,                -- temps passé dans from_state
  at            INTEGER NOT NULL
);

CREATE INDEX idx_nav_transition ON navigation_event(from_state, action);

-- mesure de l'efficacité du préchargement
CREATE TABLE preload_outcome (
  id            INTEGER PRIMARY KEY,
  key           TEXT NOT NULL,          -- ressource préchargée
  reason        TEXT NOT NULL,          -- règle ou modèle déclencheur
  used          INTEGER NOT NULL,       -- 0 = gaspillé, 1 = servi
  at            INTEGER NOT NULL
);
```

---

## 7. Moteur de synchronisation

C'est 60 % du produit. Le reste est de la présentation.

### 7.1 Trois niveaux de fraîcheur

| Tier | Contenu | Intervalle | Canal |
|---|---|---|---|
| **hot** | La PR actuellement ouverte à l'écran | 10 s | GraphQL ciblé + ETag |
| **warm** | L'inbox, les PR de l'utilisateur | `X-Poll-Interval` (≥ 60 s) | `/notifications` conditionnel |
| **cold** | Métadonnées de repos, labels, membres d'équipe | 6 h | REST conditionnel |

Le tier **hot** suit le focus : quand l'utilisateur ouvre une PR, elle passe en hot ; quand il la quitte, elle retombe en warm après 2 minutes. Au maximum **une seule** ressource en hot à la fois.

Quand la fenêtre perd le focus depuis plus de 5 minutes, tout retombe en cold et le polling passe à 5 minutes. Quand elle reprend le focus, une passe de rattrapage immédiate.

### 7.2 Rate governor

Un composant unique, obligatoire, par lequel passe **toute** requête sortante :

- Sémaphore à **8 permis** (jamais 100).
- Budget glissant : refuse de dépasser 1 500 requêtes comptabilisées par heure.
- Lit `x-ratelimit-remaining` et `x-ratelimit-reset` à chaque réponse ; sous 20 % de quota restant, bascule en mode dégradé (tier warm à 5 min, cold suspendu) et le signale dans l'UI.
- Sur 403/429 : respecte `Retry-After`, sinon backoff exponentiel avec jitter, base 1 s, plafond 15 min.
- Journalise le coût en points (GET = 1, mutation = 5) pour surveiller la limite par minute.
- Expose un état lisible par l'UI : `Healthy | Degraded | Throttled`, avec le quota restant. **L'utilisateur doit pouvoir voir pourquoi son app ralentit.**

### 7.3 Mutations optimistes

Le flux, sans exception :

1. Écrire l'état cible dans SQLite dans une transaction, avec une ligne `mutation` en `pending` et une clé d'idempotence.
2. Émettre l'événement vers le frontend. **L'UI est à jour ici, en moins de 5 ms.**
3. Le worker envoie la requête.
4. Succès → `done`, on remplace par la réponse serveur (qui peut différer).
5. Échec définitif (4xx non retryable) → rollback de l'état local, `mutation_failed` émis, et une bannière **non modale** qui explique quoi et pourquoi, avec une action « réessayer ». Jamais de dialogue bloquant.
6. Échec transitoire → retry avec backoff, l'état optimiste reste affiché avec un indicateur discret de synchronisation en attente.

Au démarrage, la file `pending` et `inflight` est rejouée. La clé d'idempotence protège du double envoi après crash.

**Invariants à tester** :
- Un crash entre l'étape 1 et 3 ne perd jamais une mutation.
- Un crash entre 3 et 4 ne duplique jamais un commentaire.
- Deux mutations concurrentes sur la même cible sont sérialisées.

### 7.4 Réconciliation

L'API fait autorité. En cas de divergence entre l'état local et l'état distant sur une entité sans mutation en attente, l'état distant écrase, silencieusement. Si une mutation est en attente sur cette entité, la mutation gagne jusqu'à sa résolution.

### 7.5 Source d'événements et priorités

Deux abstractions à poser **dès le premier jour**. Coût aujourd'hui : une heure. Coût si rétrofitées plus tard : une réécriture du scheduler.

```rust
/// D'où viennent les signaux de changement. Le scheduler ne sait pas
/// s'il polle ou s'il reçoit un push, et sait fusionner les deux
/// en dédupliquant sur (entity_id, updated_at).
#[async_trait]
pub trait EventSource: Send {
    async fn next(&mut self) -> Vec<ChangeSignal>;
}

pub struct PollingSource { /* notifications + ETag, M1 */ }
pub struct PushSource    { /* WebSocket vers le backend, M3 */ }

/// Toute requête sortante porte une priorité. Le rate governor
/// ordonne selon elle, et préempte le spéculatif.
#[derive(PartialOrd, Ord)]
pub enum Priority {
    Speculative,  // préchargement, annulable à tout moment
    Warm,         // rafraîchissement de fond
    Hot,          // entité affichée à l'écran
    User,         // conséquence directe d'une action, jamais différée
}
```

**Règle de préemption** : l'arrivée d'une requête `User` annule les requêtes `Speculative` en vol. Une action de l'utilisateur ne doit jamais attendre derrière une supposition.

### 7.6 Prédiction et préchargement

On ne peut pas rendre le réseau plus rapide. On peut faire en sorte que la donnée soit déjà là. Trois couches, à construire strictement dans cet ordre : la première capture l'essentiel du gain.

#### Couche 1 — Règles déterministes (M1)

Aucun apprentissage. À implémenter avec le sync engine :

| Déclencheur | Préchargement |
|---|---|
| L'inbox se charge | Détail complet des **5 premières** entrées |
| Une PR est ouverte | La suivante et la précédente dans la liste courante |
| Le curseur reste sur une ligne **> 150 ms** | Cette ligne. L'intention précède l'ouverture d'environ 400 ms. |
| Un diff est ouvert | Les **3 fichiers suivants** |
| Retour au premier plan | Ce qui est visible d'abord, le reste ensuite |

#### Couche 2 — Modèle comportemental local (M2)

Entièrement sur la machine, dans SQLite, alimenté par `navigation_event`. Aucun backend, donc aucune question de vie privée.

- **Chaîne de Markov d'ordre 1 sur les transitions** : depuis `(from_state, action)`, distribution des états suivants. Une table de comptage suffit, mise à jour incrémentale. Pas de réseau de neurones, pas de dépendance ML.
- **Priors temporels** : préchauffage du cache aux heures où l'utilisateur traite habituellement ses reviews.
- **Affinités** : auteurs dont les PR sont systématiquement ouvertes en premier, labels systématiquement ignorés.

#### Couche 3 — Priors agrégés (M3, optionnel)

Résout le démarrage à froid : un nouvel utilisateur reçoit les probabilités de transition moyennes de la population, anonymisées, puis son modèle local diverge vers ses propres habitudes en quelques jours. Ne remontent que des séquences de **types** d'actions, jamais un nom de dépôt, jamais de contenu. Opt-in explicite. Gain marginal faible par-dessus les couches 1 et 2 : à ne construire que si la mesure le justifie.

#### Gouvernance du budget — la partie qui compte

Un préchargement mal gouverné brûle le quota et rend l'application **plus lente**. Contraintes non négociables :

- Priorité `Speculative`, préemptable, annulée par toute action utilisateur.
- Plafond dur : **15 % du quota** maximum consacré à la spéculation. Vérifié par le rate governor, pas par le prédicteur.
- Strictement en lecture, idempotent. **Jamais une mutation.**
- Donnée préchargée mais périmée : affichée immédiatement, revalidée derrière. L'utilisateur voit en 5 ms, la correction arrive en 200 ms s'il y a lieu.
- **Auto-désactivation** : si le taux d'utilisation mesuré via `preload_outcome` passe sous **40 %** sur une fenêtre glissante de 500 préchargements, la spéculation se coupe seule et le signale dans le panneau de diagnostic.

**La métrique du produit n'est pas la précision du modèle**, c'est le *pourcentage de navigations servies depuis le cache en moins de 50 ms*. C'est ce chiffre qui est affiché en diagnostic et c'est lui qu'on optimise.

---

## 8. Modèle d'interaction

**C'est le cœur du produit.** Une fonctionnalité qui n'est pas atteignable au clavier n'est pas terminée.

### 8.1 Principes

1. **La souris est facultative partout.** Sans exception, y compris dans les dialogues.
2. **Aucun état modal invisible.** Si l'app est dans un mode, un indicateur le montre.
3. **Pas de spinner sur une lecture.** Les données viennent du cache. Si elles n'y sont pas encore, on affiche la structure avec des placeholders, jamais un écran vide.
4. **Le focus est explicite et toujours visible.** Un anneau de focus, pas une nuance de fond de 3 %.
5. **Les raccourcis sont mnémoniques et composables**, pas alphabétiques.
6. **Rien de découvrable n'échoue.** Une commande visible dans la palette ou liée à une touche est une commande qui fonctionne. Le filtrage par capacités (§3.7) s'applique au registre de commandes lui-même, pas seulement au rendu : `is_enabled(ctx)` consulte `Capabilities`.

### 8.2 Carte des touches (M1)

Navigation globale, séquences à la vim :

```
g i     aller à l'inbox
g p     aller à mes pull requests
g 1..9  aller à la vue sauvegardée n
⌘K      palette de commandes
⌘P      recherche rapide (PR, repo, personne)
?       aide contextuelle des raccourcis (affiche ce qui est valide ICI)
```

Dans une liste :

```
j / k       ligne suivante / précédente
J / K       ligne suivante / précédente ET ouvrir (navigation liée)
g g / G     début / fin
o / Enter   ouvrir
Space       aperçu sans quitter la liste
x           sélectionner (multi-sélection)
e           archiver la notification
u           marquer comme non lu
/           filtrer dans la liste courante
Esc         effacer le filtre, puis désélectionner, puis remonter
```

Dans une PR :

```
]  / [      fichier suivant / précédent
} / {       hunk suivant / précédent
n / p       thread non résolu suivant / précédent
c           commenter à la position courante
r           répondre au thread courant
R           résoudre le thread courant
v           ouvrir le panneau de soumission de review
v a         approuver et soumettre
v c         demander des changements et soumettre
V           ouvrir le fichier courant dans l'éditeur externe, à la bonne ligne
w           marquer le fichier comme vu
m           merger (avec confirmation inline, pas de dialogue)
```

Règle : **une action destructive ou publique demande une confirmation inline** (retaper la touche, ou `y`), jamais une modale qui vole le focus.

### 8.3 Palette de commandes

Ce n'est pas un menu déguisé. Exigences :

- **Contextuelle** : n'affiche que les commandes valides dans l'état courant. Un prédicat `is_enabled(ctx)` par commande.
- **Avec arguments** : `Assigner à →` ouvre un second niveau de sélection dans la même palette, sans fermeture.
- **Apprenante** : classement par fréquence d'usage récent, pondéré par le contexte.
- **Enseignante** : chaque entrée affiche son raccourci clavier à droite. La palette est le principal vecteur d'apprentissage des raccourcis.
- **Scoring fuzzy à la fzf** : bonus sur les débuts de mots et les limites de casse, pas un simple `includes()`.
- Ouverture en **moins de 30 ms**, premier résultat peint compris.

Registre typé, une seule source de vérité :

```rust
pub struct Command {
    pub id: &'static str,          // "pr.approve"
    pub title: &'static str,       // "Approuver la pull request"
    pub keywords: &'static [&'static str],
    pub binding: Option<KeyChord>,
    pub scope: Scope,              // Global | List | PullRequest | Diff
    pub is_enabled: fn(&Ctx) -> bool,
    pub run: fn(&mut Ctx) -> Result<()>,
}
```

Un test vérifie que toute commande du registre est atteignable, et qu'aucun raccourci n'est en conflit dans un même scope.

### 8.4 Vues sauvegardées, et non « templates métier »

Ne jamais coder en dur des personas. Construire **une** primitive :

```
vue = requête + colonnes + tri + groupement + raccourci
```

DSL de requête, textuel, avec autocomplétion :

```
is:pr is:open review-requested:@me -author:@me sort:updated-desc
is:pr is:open author:@me checks:failing
is:issue label:bug assignee:@me repo:org/api
```

Les « profils métier » (dev, ops, lead, release manager) ne sont alors qu'un fichier JSON de vues livrées par défaut, modifiables, exportables. C'est la seule façon d'éviter une explosion combinatoire de code.

### 8.5 Direction visuelle

Ne pas produire un énième thème sombre générique. Contraintes :

- **La densité est la fonctionnalité.** Cible : 40 lignes de liste visibles sur un écran 13". Hauteur de ligne 28 px, pas 56.
- **Un seul accent chromatique**, réservé au focus et à l'état actionnable. La couleur ailleurs sert exclusivement à encoder l'état de CI et de review (succès, échec, en attente), jamais à décorer.
- **Typographie** : une seule famille à chasse variable pour l'interface, une famille à chasse fixe pour le code et les SHA. Pas de mélange de trois graisses dans une même ligne.
- **Pas d'ombres portées, pas de cartes arrondies partout.** La hiérarchie passe par l'espacement et l'alignement. Les bordures encodent une information (limite de hunk, thread non résolu), elles ne décorent pas.
- **Mouvement** : uniquement en réponse à une action de l'utilisateur, pour montrer ce qui a changé. Aucune animation d'entrée au chargement. Respecter `prefers-reduced-motion`.
- Écrans vides : une invitation à agir avec le raccourci correspondant, jamais une illustration.

---

## 9. Feuille de route

### Jour 1 — Fondations et dérisquage

Pas d'étude préalable. Deux mesures seulement bloquent du code, parce qu'elles sont pénibles à rétrofiter. Le reste se mesure en marchant.

**Ce qui bloque et se traite en premier :**

- **J1-a** — Point de rupture de la requête GraphQL de détail de PR. La mesurer sur une PR de 5, 50 et 300 fichiers. Détermine si l'on écrit une requête unique ou paginée. Une heure.
- **J1-b** — Bench de la requête d'inbox sur un dataset synthétique (20 repos, 300 PR, 5 000 commentaires). Si le schéma du §6 ne tient pas les 5 ms, on le corrige **avant** d'écrire par-dessus. Une heure.

**Ce qui se construit ensuite, dans l'ordre :**

1. Workspace, CI (fmt, clippy `-D warnings`, test, bench), harnais `criterion` avec les budgets du §4 écrits et rouges.
2. Les abstractions du §7.5 : `EventSource` et `Priority`. Avant tout code réseau.
3. `quay-forge` : auth PAT (§3.6), rate governor (§7.2), cache ETag. **Le rate governor existe avant la première boucle de polling**, sinon la première session de développement se termine sur un 403 de limite secondaire.
4. `quay-store` : schéma du §6, migrations.
5. Le chemin de fetch complet : `/notifications` → détail de PR → SQLite.

**Ce qui tourne en tâche de fond, sans bloquer personne :** un logger de latence qui mesure sur plusieurs heures l'écart entre une action effectuée sur github.com et sa détection locale. Résultat lu le lendemain. Il n'influence aucune ligne de code du M1, seulement la décision backend du §12.

**Critère de sortie** : l'inbox de review réelle, multi-repos, s'affiche depuis SQLite en moins de 5 ms. En CLI ou dans une fenêtre Tauri, peu importe. C'est le premier moment de vérité du produit.

### M1 — Tranche verticale : l'inbox de review

Une seule boucle, du token au pixel, tenue aux budgets de la section 4.

- Auth + stockage keychain, un seul compte, avec le modèle de capacités du §3.7.
- Sync engine complet : trois tiers, rate governor, file de mutations.
- **Préchargement, couche 1** (§7.6). C'est ce qui fait la différence perçue, pas une optimisation tardive.
- Écran unique : liste d'inbox multi-repos, vue PR avec diff et threads.
- Actions : approuver, demander des changements, commenter, répondre, résoudre, archiver.
- Palette de commandes + carte des touches du §8.2.
- Vues sauvegardées avec le DSL.
- Collecte de `navigation_event`, sans consommateur pour l'instant.

**Critère de sortie** : dogfood. Traiter dix PR réelles en vingt minutes, sans souris, sans ouvrir github.com.

### M2 — Contexte local et modèle comportemental

Association repo distant ↔ clone local, checkout de la PR en un raccourci, ouverture dans l'éditeur externe à la bonne ligne, historique de fichier, blame. gitoxide entre en jeu.

Préchargement couche 2 (§7.6), alimenté par les données collectées au M1. Synchronisation de configuration chiffrée, si le backend existe.

### M3 — Élargissement

Issues, multi-comptes, GHES, mode OAuth secondaire. Backend optionnel : priors agrégés, vues d'équipe, canal de signal push pour les équipes qui installent l'App. Graphe DAG si et seulement si l'usage le confirme.

---

## 10. Ordre de construction

Strict. Chaque étape est terminée avant la suivante.

1. Workspace, CI, harnais de bench avec les budgets du §4 écrits et rouges.
2. `EventSource` et `Priority` (§7.5).
3. Mesures J1-a et J1-b. Elles peuvent invalider le schéma ou la stratégie GraphQL : les faire avant d'écrire par-dessus.
4. `quay-forge` : auth, rate governor, cache ETag.
5. `quay-store` : schéma, migrations, requête d'inbox.
6. Chemin de fetch complet jusqu'à SQLite.
7. Première sortie visible : l'inbox réelle.

Pas d'UI Tauri avant l'étape 7. Pas de vue de diff, pas de mutations, pas de palette avant que ce chemin soit propre et tenu aux budgets.

---

## 11. Règles d'ingénierie

- Toute réponse d'API est désérialisée dans un type dédié à la couche transport, jamais directement dans le type du domaine. Les deux évoluent séparément.
- Les tests du sync engine tournent contre un serveur HTTP simulé (`wiremock`) qui sait renvoyer des 304, des 403 avec `Retry-After`, des réponses tronquées et des timeouts. **Les chemins d'erreur sont testés avant les chemins heureux.**
- Les migrations SQLite sont versionnées et testées à l'aller. Pas de migration destructive sans sauvegarde du fichier.
- Aucun `.unwrap()` ni `.expect()` dans `quay-forge`, `quay-store`, `quay-sync`.
- Les secrets ne transitent jamais par l'IPC vers le frontend. Le frontend ne voit jamais un token.
- Journalisation structurée (`tracing`) avec redaction automatique des tokens. Un bouton « exporter les logs de diagnostic » dès le M1.

---

## 12. Décisions ouvertes

À trancher par l'humain, ne pas décider seul :

1. ~~Mode d'authentification~~ — **tranché**, cf. §3.6. PAT classique par défaut, OAuth device flow en secondaire. Reste à valider la matrice de capacités OAuth en M0-1.
2. **Périmètre du backend.** Tranché sur le principe (§5.5) : accélérateur optionnel, jamais dépendance, aucun token ni contenu stocké. Reste à décider quels services il rend en premier au M2, et si le canal push mérite d'exister au M3 au vu des chiffres de latence mesurés.
3. **Modèle économique**, car il contraint l'architecture. Une licence perpétuelle locale et un abonnement avec backend n'impliquent pas les mêmes choix.
4. **Cible OS prioritaire.** macOS d'abord permet d'aller plus vite sur le polish. Linux est où sont beaucoup d'ops.
5. **Nom du produit.**

---

## 13. Risques

| Risque | Impact | Mitigation |
|---|---|---|
| L'auth notifications impose le PAT classique | Onboarding dégradé, permissions trop larges | Mesuré en M0-1. Fallback : polling `search/issues` avec ETag, plus coûteux en quota |
| Latence perçue trop élevée sans backend | La thèse produit s'effondre | Mesuré en M0-2. Les mutations optimistes couvrent 90 % de la perception |
| Dérive de périmètre vers la parité github.com | Le produit ne sort jamais | Section 2, relue à chaque revue de milestone |
| Rate limit partagé avec les autres outils de l'utilisateur | Dégradation aléatoire et inexplicable | Plafond auto-imposé à 30 % du quota, état visible dans l'UI |
| GitHub change ses limites sans préavis | Le sync casse en production | Le rate governor lit toujours les headers, ne code jamais les valeurs en dur |
| Le préchargement brûle le quota et ralentit l'app | Effet inverse de celui recherché | Plafond de 15 %, priorité préemptable, auto-désactivation sous 40 % d'utilisation (§7.6) |
| Le backend devient une dépendance par glissement | Panne du VPS = produit cassé | Test d'intégration obligatoire : suite complète exécutée avec le backend injoignable |

---

## Sources

- Rate limits REST et limites secondaires : https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api
- Requêtes conditionnelles et bonnes pratiques : https://docs.github.com/en/rest/using-the-rest-api/best-practices-for-using-the-rest-api
- Notifications, `X-Poll-Interval` et contrainte d'authentification : https://docs.github.com/en/rest/activity/notifications
- Restrictions d'accès des OAuth apps, activées par défaut sur les nouvelles orgs : https://docs.github.com/en/organizations/managing-oauth-access-to-your-organizations-data/about-oauth-app-access-restrictions
- Politique de PAT au niveau organisation, approbation limitée aux tokens fine-grained : https://docs.github.com/en/organizations/managing-programmatic-access-to-your-organization/setting-a-personal-access-token-policy-for-your-organization
- Politiques de PAT au niveau entreprise, non contournables par les orgs : https://docs.github.com/en/enterprise-cloud@latest/admin/enforcing-policies/enforcing-policies-for-your-enterprise/enforcing-policies-for-personal-access-tokens-in-your-enterprise
- Permissions requises pour créer des webhooks (admin sur le dépôt, owner sur l'org) : https://docs.github.com/webhooks/about-webhooks-for-repositories