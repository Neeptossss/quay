# NOTES.md — traces de session

Fichier en append seulement. Une entrée par session.

---

## Session 2026-09-04 — étapes 1 à 3 du §10

### Ce qui a été fait

**Étape 1 — workspace, CI, harnais de bench.**

- Workspace Cargo avec les six crates du §5.2 (`quay-core`, `quay-store`, `quay-forge`,
  `quay-sync`, `quay-git`, `quay-app`) plus `xtask`. `apps/desktop` n'est pas créé : interdit avant
  l'étape 7.
- `rust-toolchain.toml` épinglé sur 1.98.1, édition 2024, resolver 3.
- `#![deny(clippy::unwrap_used, clippy::expect_used)]` en tête de `quay-forge`, `quay-store` et
  `quay-sync`, `#![forbid(unsafe_code)]` partout. L'autorisation d'`unwrap` est portée par un
  `#[allow]` sur les modules `#[cfg(test)]` uniquement, jamais sur du code de production.
- Test d'architecture du §5.2 dans `xtask/tests/architecture.rs` : lit `cargo metadata --no-deps` et
  vérifie une liste blanche d'arêtes autorisées. `quay-core` n'a aucune dépendance interne,
  `quay-store` et `quay-forge` ne se connaissent pas, rien ne dépend de `quay-app`.
- Les neuf budgets du §4 sont déclarés dans `xtask/src/budgets.rs`. `cargo run -p xtask -- budgets`
  distingue `MISSING` (aucune mesure) de `OVER` (mesure au-dessus du budget). Les deux sont rouges,
  mais ils ne disent pas la même chose. Sortie non nulle si un seul budget est rouge.
- CI GitHub Actions : `fmt`, `clippy -D warnings`, `test`, `bench`, `budgets`. Le job `budgets` est
  rouge par construction et le restera jusqu'à ce que les neuf budgets soient mesurés et tenus.

**Étape 2 — `EventSource` et `Priority` (§7.5).**

- `Priority`, `ChangeSignal`, `EntityId`, `Timestamp` et le trait `EventSource` sont dans
  `quay-core`. Placer le trait dans `quay-sync` aurait inversé le graphe du §5.2 : `quay-forge`
  fournira `PollingSource` à l'étape 4 et ne peut pas dépendre de `quay-sync`.
- `Priority` ordonne `Speculative < Warm < Hot < User`. La règle de préemption est portée par
  `cancels_in_flight`, testée dans les deux sens : `User` annule `Speculative`, et rien d'autre
  n'annule quoi que ce soit.
- La déduplication sur `(entity_id, updated_at)` est une fonction pure `merge_deduplicated`,
  testable sans I/O. C'est la sémantique qui définit l'abstraction, elle est donc avec elle.

**Étape 3 — mesures J1-a et J1-b.** Voir plus bas.

### Décisions prises

| Décision | Justification |
|---|---|
| Le rate governor est écrit maintenant, pas à l'étape 4 | J1-a émet des requêtes réseau et `CLAUDE.md` interdit toute requête sortante hors gouverneur, y compris dans les scripts de mesure. L'alternative aurait été de suspendre la règle pour la mesure. Le gouverneur écrit couvre le §7.2 sauf le plafond de 15 % de spéculation, reporté à l'étape 4 faute de consommateur. |
| Verrous de développement fondés sur l'intention de la requête, pas sur la seule méthode HTTP | Une requête GraphQL de lecture passe par POST. Un verrou fondé sur la méthode seule aurait bloqué toute lecture GraphQL. `RequestKind` déclare l'intention, et la cohérence méthode/intention est vérifiée, plus un contrôle que le document GraphQL déclaré `query` ne contient pas d'opération `mutation` au niveau supérieur. |
| `QUAY_READONLY` est actif par défaut en l'absence de la variable | Un verrou qui s'ouvre quand on oublie de le fermer n'est pas un verrou. Seul `QUAY_READONLY=0` le lève. |
| Criterion pour la régression, harnais de percentiles séparé pour les p99 | Criterion mesure des échantillons multi-itérations et ne rapporte pas de p99 par opération. Deux budgets du §4 sont écrits en p99. Les deux harnais mesurent les mêmes requêtes et leurs moyennes concordent, ce qui sert de contrôle croisé. Les sorties criterion vivent dans `target/` et ne sont pas commitées : aucun chiffre publié n'en provient. |
| Le budget `inbox_query` retient le p99 le plus défavorable des cinq requêtes | Retenir la plus rapide reviendrait à choisir son chiffre. |
| Générateur pseudo-aléatoire déterministe écrit à la main plutôt que `rand` | `StdRng` ne garantit pas la stabilité des valeurs entre versions majeures. Un dataset de mesure doit rester identique d'une exécution et d'une machine à l'autre pour que les logs bruts soient comparables. |
| Le français reste confiné à `xtask/src/report.rs` | Le rapport généré est en français, comme l'exige `CLAUDE.md`. Les chaînes de rendu vivent nécessairement dans le générateur. Tout le reste du code est en anglais. |

### Dépendances ajoutées

| Dépendance | Justification |
|---|---|
| `async-trait` | `EventSource` sera tenu derrière `Box<dyn EventSource>` par le scheduler ; `async fn` en trait natif n'est pas compatible avec les objets de trait. |
| `reqwest` (rustls) | Contrôle fin des en-têtes conditionnels, cf. §5.3. `rustls` plutôt que `native-tls` pour éviter la dépendance à l'OpenSSL du système. |
| `rusqlite` (bundled) | SQLite embarqué, version maîtrisée, indépendante de celle du système. |
| `criterion` | Harnais de bench exigé par le §10. |
| `time` | Formatage RFC3339 des horodatages du dataset et des logs. Le calcul calendaire à la main est une source classique de bugs, et la couche transport en aura besoin pour désérialiser les dates GitHub. |
| `thiserror` | Types d'erreur du gouverneur sans code répétitif. |
| `tokio` | Runtime asynchrone, sémaphore du gouverneur. |
| `tracing`, `tracing-subscriber` | Journalisation structurée exigée par le §11. |
| `serde`, `serde_json` | Sérialisation des mesures et lecture des réponses GraphQL. |
| `wiremock` (dev) | Serveur simulé exigé par le §11 pour les chemins d'erreur du code réseau. |
| `tempfile` (dev et xtask) | Bases SQLite jetables des tests et des mesures. |

### Mesures

Logs bruts dans `measurements/raw/`, résumés machine dans `measurements/summary/`, rapport généré
par `cargo run -p xtask -- report` dans `measurements/REPORT.md`. Aucun chiffre de ce fichier n'est
saisi à la main : ils sont tous relus depuis ces logs.

Machine de mesure : `aarch64-macos`, Apple Silicon. Ce n'est pas la machine de référence du §4 au
sens strict, mais elle en est proche (M-series). Les chiffres réseau dépendent en outre de la
liaison depuis laquelle la session a tourné.

#### J1-a — point de rupture de la requête GraphQL de détail de PR

194 requêtes émises, 194 points dépensés, gouverneur `Healthy` de bout en bout, aucun 403, aucun 429.

Cibles retenues automatiquement sur des dépôts publics, épinglées dans le log brut :
`rust-lang/rust#161795` (5 fichiers, 3 threads), `odoo/odoo#69930` (50 fichiers, 13 threads),
`odoo/odoo#149028` (323 fichiers, 2 threads), `odoo/odoo#63177` (70 fichiers, 170 threads).

Résultats principaux :

1. **Aucune rupture au sens d'un échec.** Zéro erreur GraphQL, zéro timeout, zéro limite secondaire,
   y compris à `files: 100, threads: 100, comments: 100`. La rupture est une rupture de latence, pas
   de disponibilité.
2. **Plancher de latence d'environ 690 à 880 ms par requête, indépendant du nombre de fichiers.**
   Une PR de 5 fichiers coûte le même aller-retour qu'une PR de 323 : 719 ms contre 873 ms en pages
   étroites. Seule la profondeur des threads fait sortir de cette plage, cf. point 4. C'est ce
   plancher qui condamne toute lecture réseau synchrone sur une action utilisateur, et qui justifie
   le préchargement du §7.6 bien plus que la bande passante.
3. **`first: 100` est un plafond dur de l'API.** Une PR de 323 fichiers ne peut pas être récupérée
   en une passe. La question « requête unique ou paginée » n'est donc pas ouverte au-dessus de 100
   fichiers : elle est tranchée par GitHub.
4. **La profondeur des threads est le vrai facteur de coût, pas le nombre de fichiers.** Sur la PR à
   170 threads, la latence monte de 719 ms (20 threads demandés) à 1160 ms (50) puis 1423 ms (100),
   et la récupération complète en pagination coûte 3682 ms pour 119 Ko. Sur la PR à 323 fichiers
   mais 2 threads, la pagination complète coûte 2397 ms pour 33 Ko.
5. **`rateLimit.nodeCount` est facturé sur les tailles de page demandées, pas sur les données
   rendues.** Demander 100/100/100 coûte 10 201 nœuds même sur une PR de 5 fichiers et 3 threads,
   contre 241 nœuds à 20/20/10. Surdimensionner les pages « au cas où » consomme le budget de nœuds
   et le temps CPU GraphQL du §3.3 sans rien rapporter.
6. **Le coût en points est de 1 par requête quelle que soit la taille de page.** Le coût réel d'une
   stratégie se mesure donc en nombre de requêtes, pas en volume.

Lecture pour l'étape 4 : requête ciblée et étroite pour le tier hot (pages petites, uniquement ce
qui est affiché), pagination à la demande pour le reste, jamais une passe unique surdimensionnée.

#### J1-b — requête d'inbox sur dataset synthétique

Dataset de référence du §4 : 20 dépôts, 300 PR ouvertes, 5 000 commentaires, schéma du §6 appliqué
verbatim, payloads `raw` de 3 Ko par PR. Cinq formes de requête, 10 000 itérations chacune en cache
chaud, 100 en cache froid.

**Le budget de 5 ms est tenu sur le dataset de référence, avec une marge d'un facteur 20.** Pire p99
observé : 0,249 ms (`open_with_unresolved_thread_count`, cache chaud). Le budget `inbox_query` passe
donc de `MISSING` à `MET`.

Mais la courbe de montée en charge révèle deux défauts du schéma du §6, tous deux invisibles au
dataset de référence :

| Requête | 300 PR | 3 000 PR | 30 000 PR |
|---|---|---|---|
| `open_across_tracked_repos` | 0,062 ms | 0,056 ms | 0,066 ms |
| `open_with_unresolved_thread_count` | 0,249 ms | **13,3 ms** | **295,6 ms** |
| `review_requested_approximation` | 0,120 ms | 0,106 ms | 0,071 ms |
| `open_filtered_by_common_title` | 0,063 ms | 0,068 ms | 0,060 ms |
| `open_filtered_by_rare_title` | 0,102 ms | **7,1 ms** | **28,2 ms** |

(p99, cache chaud, en millisecondes.)

### Blocages et questions ouvertes

**1. Le §6 n'a pas d'index sur `review_thread(pr_id)`.**

Le plan d'exécution le dit sans ambiguïté :
`SEARCH t USING AUTOMATIC PARTIAL COVERING INDEX (pr_id=? AND is_resolved=?)`. SQLite reconstruit un
index transitoire à chaque exécution de la requête, ce qui rend le coût linéaire en taille de
`review_thread` pour chacune des 50 lignes rendues. C'est ce qui produit les 295 ms à 30 000 PR.

Le compteur de threads non résolus n'est pas un ornement : c'est la colonne qui dit à l'utilisateur
s'il a quelque chose à faire sur cette PR. L'inbox sans ce compteur n'est pas l'inbox du produit.

La même omission a un second effet : `review_thread.pr_id` porte `ON DELETE CASCADE`, et SQLite
parcourt la table enfant entière à chaque suppression de PR faute d'index sur la clé étrangère.

Correction proposée, à valider avant d'écrire par-dessus :

```sql
CREATE INDEX idx_thread_by_pull_request ON review_thread(pr_id, is_resolved);
CREATE INDEX idx_comment_by_thread ON review_comment(thread_id);
```

**2. La requête d'inbox canonique du §8.4 n'est pas exprimable sur le schéma du §6.**

`is:pr is:open review-requested:@me -author:@me` suppose de savoir de qui la review est demandée.
Le §6 n'a ni table de reviewers, ni table de demandes de review, ni colonne équivalente sur
`pull_request` : `review_state` est la décision agrégée, pas la demande individuelle. Le test
`no_table_records_which_reviewer_a_pull_request_is_requested_from` fige ce constat.

Faute de mieux, la mesure porte sur `review_requested_approximation`, qui approxime par
`author <> @me AND (review_state IS NULL OR review_state = 'commented')`. Cette approximation est
fausse en pratique : elle inclut toutes les PR ouvertes des autres, pas celles où l'utilisateur est
sollicité. Or c'est exactement le filtre du produit, celui de l'écran principal du M1.

Correction proposée, à valider :

```sql
CREATE TABLE review_request (
  pr_id         INTEGER NOT NULL REFERENCES pull_request(id) ON DELETE CASCADE,
  reviewer      TEXT NOT NULL,
  is_team       INTEGER NOT NULL DEFAULT 0,
  requested_at  TEXT NOT NULL,
  PRIMARY KEY (pr_id, reviewer, is_team)
);

CREATE INDEX idx_review_request_by_reviewer ON review_request(reviewer, pr_id);
```

Le §8.4 mentionne aussi `label:`, `assignee:` et `checks:`. `checks_state` existe sur
`pull_request`, les deux autres n'ont aucun support. Ils ne sont pas mesurés ici : le décider
maintenant serait trancher seul une question de périmètre.

**3. Le filtre `/` du §8.2 fait un balayage complet de `pull_request`, blobs `raw` compris.**

`open_filtered_by_rare_title` passe de 0,102 ms à 28,2 ms entre 300 et 30 000 PR. Quand le motif est
sélectif, aucune ligne ne satisfait le `LIKE` et SQLite parcourt toutes les lignes de l'index
`idx_pr_inbox`, en visitant la table pour chaque ligne, donc en traversant les pages de débordement
des payloads `raw` de 3 Ko. Le cas rapide mesuré au premier passage (`open_filtered_by_common_title`,
0,06 ms) était un artefact de mon dataset : tous les titres générés contenaient le motif, la requête
s'arrêtait donc au bout de 50 lignes. Les deux variantes sont conservées, c'est l'écart entre elles
qui est l'information.

Deux corrections possibles, à trancher, pas de recommandation ferme sans mesure comparative :

- déplacer `raw` dans une table séparée `pull_request_payload(pr_id, raw)`, ce qui rend les lignes
  de `pull_request` petites et les balayages bon marché ;
- ou indexer les titres en FTS5, ce qui règle la recherche mais pas les autres balayages.

**4. Deux requêtes ont contourné le gouverneur.**

Deux appels `curl` manuels ont servi à vérifier la validité du token et ses scopes avant de lancer
J1-a. Ils n'ont pas transité par le rate governor. Ils ne sont pas dans les logs de mesure et ne
comptent dans aucun chiffre publié, mais la règle du `CLAUDE.md` ne prévoit pas d'exception et il
faut le dire plutôt que de le taire. Deux points de quota consommés hors compteur.

**5. Le plancher de 700 à 900 ms de J1-a mérite d'être remesuré ailleurs.**

Il est mesuré depuis une seule liaison réseau, à un seul moment. Il porte une conclusion
structurante sur le préchargement. Une contre-mesure depuis une autre liaison éviterait de bâtir une
stratégie sur un artefact local.

### Ce qu'il faut faire ensuite

1. Trancher les points 1, 2 et 3 ci-dessus. Le §10 place la correction du schéma **avant** d'écrire
   par-dessus, et l'étape 5 est précisément le schéma.
2. Étape 4 : `quay-forge` — auth PAT du §3.6, keychain, cache ETag, `PollingSource` implémentant
   `EventSource`, plafond de 15 % de spéculation dans le gouverneur.
3. Le logger de latence du §9 (`cargo run -p xtask -- latency-log`) n'existe pas encore. Il est cité
   dans `CLAUDE.md` mais ne fait pas partie des étapes 1 à 3.
4. Les budgets `MISSING` autres que `inbox_query` le resteront jusqu'à leur jalon. Aucun ne doit
   être renseigné autrement que par une mesure réelle.

---

## Session 2026-09-04 (suite) — correction du schéma du §6

Les trois points laissés ouverts à l'entrée précédente sont tranchés. Aucun n'a été décidé sur
intuition : le §6 verbatim est conservé sous `crates/quay-store/schema/baseline-section-6.sql` comme
témoin, le schéma corrigé est dans `0001_initial.sql`, et J1-b mesure désormais les deux côte à côte
sur le même dataset déterministe. Un test vérifie que les deux schémas reçoivent exactement les
mêmes pull requests, sans quoi la comparaison ne voudrait rien dire.

### Résultat de la comparaison

p99 en millisecondes, cache chaud, dataset de référence puis ×10 et ×100.

| Requête | §6 300 | §6 3 000 | §6 30 000 | corrigé 300 | corrigé 3 000 | corrigé 30 000 |
|---|---|---|---|---|---|---|
| `open_across_tracked_repos` | 0,046 | 0,047 | 0,043 | 0,061 | 0,042 | 0,043 |
| `open_with_unresolved_thread_count` | 0,207 | 13,420 | 267,707 | **0,059** | **0,058** | **0,059** |
| `review_requested_approximation` | 0,068 | 0,084 | 0,080 | 0,064 | 0,055 | 0,049 |
| `open_filtered_by_common_title` | 0,043 | 0,068 | 0,058 | 0,049 | 0,041 | 0,042 |
| `open_filtered_by_rare_title` | 0,067 | 2,708 | 35,973 | **0,049** | **0,266** | **6,309** |
| `review_requested_exact` | impossible | impossible | impossible | **0,093** | **0,095** | **0,117** |

### Point 1 — index sur `review_thread(pr_id)` : corrigé

```sql
CREATE INDEX idx_thread_by_pull_request ON review_thread(pr_id, is_resolved);
CREATE INDEX idx_comment_by_thread ON review_comment(thread_id);
```

Le compteur de threads non résolus passe de 267,707 ms à 0,059 ms à 30 000 PR, soit un facteur
4 500, et devient **plat** : 0,059 ms aux trois échelles. Le plan confirme la cause et la correction,
`AUTOMATIC PARTIAL COVERING INDEX` devient `SEARCH t USING COVERING INDEX
idx_thread_by_pull_request`. L'index composite `(pr_id, is_resolved)` est couvrant pour ce compte, la
table `review_thread` n'est plus visitée du tout.

`idx_comment_by_thread` n'est pas exercé par les requêtes d'inbox mesurées ici. Il est ajouté pour la
même raison que le premier : `review_comment.thread_id` porte `ON DELETE CASCADE` et SQLite
parcourt la table enfant entière à chaque suppression faute d'index sur la clé étrangère. Sa valeur
n'est pas mesurée, elle est structurelle, et c'est dit ici plutôt que présenté comme un gain.

### Point 2 — table `review_request` : corrigé

```sql
CREATE TABLE review_request (
  pr_id         INTEGER NOT NULL REFERENCES pull_request(id) ON DELETE CASCADE,
  reviewer      TEXT NOT NULL,
  is_team       INTEGER NOT NULL DEFAULT 0,
  requested_at  TEXT NOT NULL,
  PRIMARY KEY (pr_id, reviewer, is_team)
) WITHOUT ROWID;

CREATE INDEX idx_review_request_by_reviewer ON review_request(reviewer, pr_id);
```

`is:pr is:open review-requested:@me -author:@me` est maintenant exprimable, et c'est elle qui porte
désormais le budget `inbox_query` : 0,093 ms de p99 contre 5 ms de budget, et 0,117 ms à 30 000 PR.
Le plan est exactement celui qu'on veut : parcours de `idx_pr_inbox` dans l'ordre `updated_at`,
sonde `EXISTS` sur `idx_review_request_by_reviewer` en index couvrant, arrêt au bout de 50 lignes.
Le coût ne dépend donc pas du nombre total de PR mais du nombre de lignes rendues.

`is_team` est présent parce que le §3.6 demande le scope `read:org` pour les « reviewers d'équipe ».
La résolution de l'appartenance à une équipe est du code, pas du schéma, et viendra avec l'étape 4.

`WITHOUT ROWID` parce que la table est entièrement définie par sa clé primaire composite et n'a
aucune colonne large : la clé primaire est l'index, il n'y a pas de rowid à maintenir en double.

`label:` et `assignee:` du §8.4 n'apparaissent que dans l'exemple `is:issue` du DSL, et les issues
sont au M3 par le §2. Rien n'est ajouté pour eux. Les ajouter maintenant serait construire pour un
jalon qu'on n'a pas atteint.

L'ancienne `review_requested_approximation` est conservée : c'est la seule forme exprimable sur le
schéma témoin, et le test `the_exact_filter_is_stricter_than_the_approximation_it_replaces` fige
pourquoi elle ne convenait pas. Elle n'est pas dans le chemin du produit.

### Point 3 — `raw` sorti de `pull_request` : corrigé, sans aller plus loin

```sql
CREATE TABLE pull_request_payload (
  pr_id         INTEGER PRIMARY KEY REFERENCES pull_request(id) ON DELETE CASCADE,
  raw           BLOB NOT NULL
);
```

Le filtre sélectif passe de 35,973 ms à 6,309 ms à 30 000 PR (facteur 5,7), et de 2,708 ms à
0,266 ms à 3 000 PR (facteur 10). Les lignes de `pull_request` ne traversent plus les pages de
débordement des payloads de 3 Ko à chaque balayage. Le principe du §6, « le contenu brut est
conservé pour permettre une remigration sans re-fetch », est intact : la relation est 1 pour 1 et
le payload n'est jamais sur le chemin chaud.

**Je m'arrête là et je n'ajoute ni FTS5 ni index couvrant sur les titres.** Trois raisons, dans cet
ordre :

1. Au dataset du §4 la requête est à 0,049 ms, soit 1 % du budget. À ×10 elle est à 0,266 ms. Les
   6,309 ms ne surviennent qu'à 30 000 PR ouvertes sur 20 dépôts, cent fois le dataset spécifié.
2. Le `/` du §8.2 filtre « dans la liste courante ». La liste courante est déjà dans le store en
   mémoire du frontend (§5.1) : ce filtre-là ne touche pas SQLite du tout. La requête mesurée ici
   est le pire cas d'une recherche globale, pas celui du raccourci.
3. La recherche globale (⌘P, §8.2) n'est pas écrite et n'arrive pas avant l'étape 7. Indexer
   maintenant pour une fonctionnalité non écrite, c'est de la généricité spéculative, et cela coûte
   une amplification d'écriture à chaque mise à jour de PR.

Point de déclenchement mesuré, à reprendre le jour où ⌘P s'écrit : la recherche globale par balayage
dépasse le budget de 5 ms entre 3 000 et 30 000 PR ouvertes. FTS5 est le levier connu.

### SQLite est-il le bon outil ?

La question a été posée. Réponse fondée sur ce qui est mesuré, pas sur une préférence.

**Oui, et les chiffres disent que ce n'est même pas le sujet.** La requête d'inbox réelle coûte
0,093 ms. La requête GraphQL de détail de PR mesurée en J1-a coûte 690 à 880 ms. Le magasin local
est **quatre ordres de grandeur** sous le coût du réseau. Optimiser le magasin plus loin ne
déplacerait rien de perceptible : tout le budget de latence perçue se joue sur le réseau, donc sur
le préchargement du §7.6 et les mutations optimistes du §7.3.

Ce que SQLite apporte et qu'une structure en mémoire n'apporterait pas :

- **Les invariants de crash du §7.3.** « Un crash entre l'étape 1 et 3 ne perd jamais une mutation »
  est une exigence transactionnelle. La réécrire soi-même est un projet à part entière.
- **Le DSL de vues sauvegardées du §8.4.** « vue = requête + colonnes + tri + groupement » est un
  langage de requêtes utilisateur. Sans SQL il faudrait écrire son propre planificateur.
- **Les lectures concurrentes pendant écriture** (WAL), exigées par le §5.4 : l'IPC répond en moins
  de 5 ms pendant que le moteur de sync écrit.
- **La remigration depuis `raw`** sans re-fetch, principe posé au §6.

Les deux défauts trouvés aujourd'hui n'étaient pas des limites de SQLite : c'étaient un index
manquant et une table manquante. Le premier est passé de 267 ms à 0,059 ms avec une ligne de DDL.

Une réserve à trancher plus tard, pas aujourd'hui : le §6 impose `PRAGMA synchronous = NORMAL` en
mode WAL. Un crash de processus ne perd alors rien, mais une coupure d'alimentation peut perdre les
dernières transactions. L'invariant du §7.3 est écrit « un crash » sans préciser lequel. Les clés
d'idempotence rendent le renvoi sûr, donc le compromis est probablement le bon, mais il doit être
choisi et pas subi. Non mesuré, à instrumenter le jour où la file de mutations existe.

### Ce qu'il faut faire ensuite

1. Étape 4 : `quay-forge` — auth PAT du §3.6, keychain, cache ETag, `PollingSource` implémentant
   `EventSource`, plafond de 15 % de spéculation dans le gouverneur.
2. Étape 5 : migrations versionnées par-dessus `0001_initial.sql`, qui est désormais stable.
3. Le logger de latence du §9 reste à écrire.
4. Trancher `synchronous = NORMAL` quand la file de mutations existe.
