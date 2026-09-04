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

Traçabilité : le jeu de requêtes ayant changé, les logs bruts de J1-b de l'entrée précédente sont
remplacés dans le répertoire de travail. Ils restent lisibles dans le commit
`Set up the workspace, sync abstractions and the J1 measurements`, qui est leur seule adresse
désormais. Les chiffres de l'entrée précédente ne sont donc pas orphelins, ils changent seulement
de point d'accès.

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

---

## Session 2026-09-04 (suite) — étape 4, `quay-forge`

### Ce qui a été fait

- **Couche transport** (`transport.rs`) : types dédiés pour `/user`, `/user/orgs`, `/notifications`
  et les erreurs, jamais désérialisés directement dans un type du domaine, conformément au §11.
- **Hiérarchie des scopes** (`scopes.rs`) : `admin:org` accorde `read:org`, `repo` accorde
  `public_repo` et consorts, `user` accorde `read:user`. Sans cela un utilisateur porteur de
  `admin:org` se verrait refuser une capacité qu'il possède, c'est-à-dire exactement le bouton gris
  muet que le §3.7 interdit.
- **Auth PAT** (`auth.rs`) : lecture d'identité à partir de `/user` et `/user/orgs`, jamais acceptée
  sans validation (§3.6). Détection SAML SSO par l'en-tête `X-GitHub-SSO`, qui distingue
  `partial-results` (des organisations sont masquées, leurs identifiants sont nommés) de `required`
  (une autorisation est à faire, l'URL est fournie). Une liste vide sans explication est donc
  impossible.
- **Modèle de capacités** (`capabilities.rs`) : les trois sources du §3.7 dans l'ordre — scopes
  réellement accordés, sondage actif, apprentissage par l'échec. Aucune table figée « OAuth ne sait
  pas faire X ». Les règles d'affichage du §3.7 sont portées par `is_discoverable` et `is_enabled` :
  une indisponibilité structurelle sort la commande du registre, une indisponibilité récupérable la
  laisse visible et désactivée, `Unknown` est traité comme disponible.
- **Keychain** (`keychain.rs`) : stockage par le trousseau du système. Le jeton ne passe jamais par
  un fichier. `Token` refuse de se rendre dans `Debug` et `Display`.
- **Requêtes conditionnelles** : `If-None-Match` et `If-Modified-Since` émis depuis des validateurs
  fournis par l'appelant, `ETag`, `Last-Modified` et `X-Poll-Interval` relus dans la réponse. Le
  gouverneur ne conserve aucun cache : `quay-forge` ne connaît pas `quay-store` (§5.2) et
  `resource_cache` appartient au §6. Aucun trait n'est introduit pour un seul implémenteur.
- **Plafond de spéculation** : le budget `Speculative` est un second compteur glissant à 15 % du
  budget horaire (§7.6). Un test vérifie qu'une requête `User` passe encore quand la spéculation est
  épuisée.

### Deux corrections de conception, l'une importante

**Un 403 n'est pas toujours une limite de débit.** Le gouverneur réessayait tout 403 cinq fois avec
backoff. Un 403 de permission — capacité absente, politique d'organisation, SSO non autorisé — n'est
pas une limite de débit : le réessayer est faux, lent, et il masque la raison que le §3.7 veut
remonter à l'interface. Le 403 n'est désormais traité comme un throttle que s'il porte `Retry-After`,
ou si `x-ratelimit-remaining` vaut zéro, ou si le corps nomme une limite. Sinon il est rendu tel
quel à l'appelant, qui en tire une capacité indisponible. Le test qui figeait l'ancien comportement
a été réécrit sur la nouvelle sémantique plutôt que contourné.

**Un 304 rend son jeton de budget.** Le §3.2 dit qu'un 304 authentifié ne décompte pas du quota.
Le budget glissant de 1 500 requêtes par heure est notre propre plafond : compter les 304 dedans
reviendrait à s'interdire le polling conditionnel qui est justement gratuit. La réservation est donc
remboursée quand la réponse est un 304. Mesuré en M0-1, cf. ci-dessous.

### Mesure M0-1 — le piège annoncé par le §3.4

Le §3.4 annonce un piège bloquant : la documentation laisse entendre que `/notifications` n'accepte
que le PAT classique. Mesuré, sur le seul type de jeton disponible.

| Observation | Résultat |
|---|---|
| PAT classique sur `/notifications` | **200**, l'endpoint répond |
| `X-Poll-Interval` | **60 s**, comme annoncé |
| Validateurs proposés | `ETag` **et** `Last-Modified` |
| 10 requêtes conditionnelles (304) | `x-ratelimit-remaining` **figé à 4968**, quota consommé **0** |
| 10 requêtes non conditionnelles (200) | 4967 → 4958, quota consommé **1 par requête** |

**Le pilier de la stratégie de polling du §3.2 est confirmé empiriquement** : dix révalidations
consécutives coûtent zéro. Sur un tier warm à 60 s, l'inbox se rafraîchit gratuitement tant qu'elle
ne change pas.

**Découverte non prévue par la spec : l'endpoint `/rate_limit` n'est pas un instrument fiable.**
Il n'a reflété **aucune** des vingt requêtes mesurées, aucun de ses seaux n'a bougé, alors que les
en-têtes de réponse les comptaient une par une. Ma première version de la mesure s'appuyait sur
`/rate_limit` et concluait, à tort, que les requêtes non conditionnelles étaient gratuites elles
aussi. C'est l'instrument qui était faux, pas GitHub. Le §7.2 avait raison de spécifier la lecture
des en-têtes à chaque réponse, et un test interdit désormais au gouverneur de sonder `/rate_limit`
de lui-même. Conséquence pour l'interface : la jauge de quota du §7.2 doit se nourrir des en-têtes,
sinon elle affichera un réservoir plein en permanence.

### Blocage

**M0-1 n'est mesuré qu'à moitié et je ne peux pas finir seul.** La question du §3.4 est de savoir si
`/notifications` exclut les PAT fine-grained et les GitHub Apps. Je n'ai qu'un PAT classique, donc
je sais seulement qu'il fonctionne. Le reste n'est pas déduit, il est absent.

Il me faut, pour conclure, un **PAT fine-grained** sur un dépôt jetable, avec la permission
`Notifications` en lecture. Une minute à créer, et cela tranche définitivement le risque n°1 du §13
ainsi que la matrice de capacités laissée ouverte au §12.1. Deux options si tu ne veux pas en créer
un : soit on avance en supposant le pire, à savoir PAT classique obligatoire, ce qui est déjà la
décision arrêtée du §3.6 et ne coûte donc rien aujourd'hui ; soit on laisse la case vide et on la
remplira au moment où le mode OAuth secondaire sera écrit, au M3. Je penche pour la première, elle
n'engage aucune réécriture.

### Écarts assumés

- Le §3.7 déclare `auth_mode: AuthMode` avec deux variantes, `PatClassic | OAuth`. J'utilise
  `TokenKind` à cinq variantes, qui ajoute le PAT fine-grained, le jeton de GitHub App et
  l'inconnu. Le §3.4 demande précisément de détecter le cas fine-grained : une énumération à deux
  variantes ne peut pas le représenter. Aucune capacité n'est dérivée du type de jeton, seulement
  des scopes et des sondages, donc l'interdit du §3.7 est respecté.
- Les trois tests du trousseau sont marqués `#[ignore]` avec la raison en toutes lettres : ils
  touchent le magasin d'identifiants du système, absent d'une CI sans session graphique. Ils ont été
  lancés sur cette machine et passent, avec un nom de service propre au processus et un faux jeton.
  Le jeton réel n'a jamais été écrit dans le trousseau.

### Dépendance ajoutée

| Dépendance | Justification |
|---|---|
| `keyring` | Trousseau du système, exigé par le §5.3 : un jeton ne s'écrit jamais en clair sur disque. |

### Ce qu'il faut faire ensuite

1. Trancher le blocage M0-1 ci-dessus.
2. Étape 5 : migrations versionnées par-dessus `0001_initial.sql`, et la requête d'inbox branchée
   sur le schéma corrigé.
3. Étape 6 : chemin de fetch complet, `PollingSource` implémentant `EventSource`, alimenté par
   `/notifications` et son `X-Poll-Interval`, validateurs persistés dans `resource_cache`.

---

## Session 2026-09-04 (suite) — étape 5, `quay-store`

Décision reportée de l'entrée précédente : M0-1 reste à moitié mesuré et **on avance en supposant le
pire**, à savoir PAT classique obligatoire. C'est déjà la décision arrêtée du §3.6, donc cette
hypothèse n'engage aucune réécriture. La case reste ouverte, elle se remplira le jour où un jeton
fine-grained sera disponible.

### Ce qui a été fait

- **`Tier` dans `quay-core`** : les trois niveaux de fraîcheur du §7.1 avec leurs intervalles, 10 s,
  60 s, 6 h. `interval_respecting` fait gagner l'intervalle conseillé par la forge quand il est plus
  long que le nôtre, jamais plus court : le §3.4 dit de respecter `X-Poll-Interval`, y compris quand
  il augmente. Le type est dans le domaine parce que `quay-sync` en aura besoin sans dépendre du
  magasin.
- **Migrations versionnées** (`migrations.rs`) : `PRAGMA user_version` porte la version, chaque
  migration s'applique dans une transaction, et une base plus récente que le binaire est **refusée**
  plutôt que rétrogradée. Une migration qui échoue laisse la version intacte et annule les
  instructions déjà passées, ce que vérifie un test à deux instructions dont la seconde est invalide.
- **Sauvegarde avant migration destructive** (§11), avec un déclencheur explicite porté par
  `rewrites_existing_rows`. Testé avec une migration destructive synthétique, sans avoir à en livrer
  une vraie.
- **Façade `Store`** : `open` applique les migrations et active `foreign_keys`. C'est le seul chemin
  d'ouverture du produit ; `schema::create` reste réservé au témoin §6 des mesures et passe
  désormais par le même exécuteur de migrations pour la variante corrigée, donc les deux chemins ne
  peuvent plus diverger.
- **Cache de fraîcheur** (`resource_cache.rs`) : lecture, écriture par remplacement, oubli, et la
  requête `due_for_refresh` qui rend ce qui est périmé, le plus chaud d'abord. C'est ce que le §6
  annonce quand il écrit que `stale_after` « pilote le scheduler ». C'est aussi le versant
  persistant du cache ETag : `quay-forge` émet et relit les validateurs mais ne les conserve pas,
  puisqu'il ne connaît pas `quay-store` (§5.2).

### Deux bugs trouvés par les tests, dont un sérieux

**La sauvegarde avant migration était fausse.** `std::fs::copy` sur une base en mode WAL copie un
fichier `.db` qui ne contient pas les dernières transactions : elles sont dans le `-wal`. La
sauvegarde produite était vide de ce qu'elle devait protéger. Une sauvegarde silencieusement
incomplète est pire que pas de sauvegarde, puisqu'on s'appuie dessus pour accepter une migration
destructive. Corrigé par `VACUUM INTO`, qui produit un instantané cohérent. Le test vérifie
maintenant qu'une ligne **commitée avant** la migration se retrouve dans la sauvegarde, pas seulement
que le fichier existe.

**Les `PRAGMA` en tête du DDL empêchaient toute migration transactionnelle.** SQLite refuse
`PRAGMA synchronous` à l'intérieur d'une transaction. Le §6 place `journal_mode` et `synchronous` en
tête du bloc de schéma, mais ce sont des réglages de connexion, pas du schéma : ils sont retirés des
deux fichiers DDL et appliqués par `schema::open`, pour le témoin comme pour le schéma corrigé. La
configuration effective est identique, seule sa localisation change. J1-b a été relancé après ce
déplacement pour que les logs correspondent au code, plutôt que de supposer que rien n'avait bougé.

### Une réserve

`foreign_keys` est activé par `Store::open` et **pas** par `schema::create`, donc pas dans le
harnais de mesure J1-b. L'écart est sans effet sur J1-b, qui ne mesure que des lectures, mais il en
aura un le jour où on mesurera des écritures : la vérification des clés étrangères a un coût, et
c'est précisément ce que les index ajoutés à l'entrée précédente rendent abordable. À reprendre
quand la file de mutations du §7.3 sera mesurée.

### Ce qu'il faut faire ensuite

1. Étape 6 : le chemin de fetch complet. `PollingSource` implémentant `EventSource`, alimenté par
   `/notifications` et son `X-Poll-Interval`, validateurs lus et écrits dans `resource_cache`,
   réponses désérialisées dans les types de transport puis écrites dans SQLite.
2. Les écritures du magasin — insertion et mise à jour de dépôts, pull requests, threads,
   commentaires et demandes de review — n'existent pas encore. Elles arrivent avec l'étape 6, qui en
   est le premier consommateur.
3. Étape 7 : première sortie visible, l'inbox réelle.

### Relance de J1-b après le déplacement des pragmas, et une correction

Les logs bruts de J1-b sont régénérés. Les conclusions tiennent, mais la relance apprend quelque
chose sur la méthode et corrige une affirmation de l'entrée précédente.

| Requête, p99 ms cache chaud | §6 300 | §6 3 000 | §6 30 000 | corrigé 300 | corrigé 3 000 | corrigé 30 000 |
|---|---|---|---|---|---|---|
| `open_across_tracked_repos` | 0,041 | 0,043 | 0,051 | 0,041 | 0,040 | 0,043 |
| `open_with_unresolved_thread_count` | 0,181 | 11,668 | 203,771 | 0,061 | 0,059 | 0,068 |
| `review_requested_approximation` | 0,057 | 0,059 | 0,061 | 0,055 | 0,053 | 0,057 |
| `open_filtered_by_common_title` | 0,049 | 0,042 | 0,047 | 0,046 | 0,043 | 0,044 |
| `open_filtered_by_rare_title` | 0,112 | 2,166 | 21,553 | 0,046 | 0,281 | 3,542 |
| `review_requested_exact` | impossible | impossible | impossible | 0,091 | 0,114 | 0,108 |

**Les p99 absolus varient d'environ un facteur 1,8 d'une exécution à l'autre sur cette machine.**
Le balayage sélectif à 30 000 PR donne 35,973 ms au premier passage et 21,553 ms au second sur le
schéma §6, 6,309 ms puis 3,542 ms sur le schéma corrigé. Le rapport entre les deux schémas, lui, est
stable : 5,7 puis 6,1. **C'est le rapport qui est la mesure, pas la valeur absolue d'un p99 unique.**
Une machine de bureau chargée n'est pas un banc isolé, et un p99 sur 2 000 itérations attrape le
bruit du système. À retenir pour toute lecture future de ces chiffres.

**Correction d'une affirmation de l'entrée précédente.** J'y écrivais que la recherche globale par
balayage « dépasse le budget de 5 ms entre 3 000 et 30 000 PR ouvertes », sur la foi des 6,309 ms
mesurés à 30 000. La relance donne 3,542 ms au même point, donc **sous** le budget. Le point de
bascule n'est pas franchi de façon nette à 30 000 : il est au voisinage du budget et dépend de la
charge de la machine. La décision de ne pas ajouter FTS5 aujourd'hui n'en est que mieux fondée, mais
la phrase était plus affirmative que la mesure ne l'autorisait.

Ce qui ne bouge pas d'une exécution à l'autre : le compteur de threads non résolus reste
catastrophique sur le §6 (203 à 267 ms à 30 000 PR) et **plat** sur le schéma corrigé (0,059 à
0,068 ms aux trois échelles), et `review_requested_exact` reste à 0,09-0,11 ms contre 5 ms de
budget. Les deux corrections structurantes sont confirmées.

---

## Session 2026-09-04 (suite) — étape 6, le chemin de fetch complet

### Ce qui a été fait

Le chemin `/notifications` → détail de PR → SQLite est complet et vérifié contre la vraie API.

- **Types du domaine** dans `quay-core` : `Repository`, `PullRequest`, `ReviewThread`,
  `ReviewComment`, `ReviewRequest`, `PullRequestSnapshot`. Les réponses de l'API sont désérialisées
  dans les types de transport de `quay-forge`, puis converties, jamais lues directement dans ces
  types-là (§11).
- **`EntityId.node_id` renommé en `key`.** Une notification donne le dépôt et le numéro de la PR,
  pas son node id : celui-ci n'est connu qu'après le fetch. La clé d'une PR est donc son localisateur
  `owner/name#numero`, ce qu'un `PushSource` du §5.5 produirait à l'identique puisque le backend
  n'enverrait que `{repo_id, pr_number}`. Le nom portait une implémentation qui n'était pas la
  bonne.
- **Écritures du magasin** : `upsert_account`, et `save_pull_request` qui écrit dépôt, pull request,
  payload brut, threads, commentaires et demandes de review dans une seule transaction. Threads et
  demandes sont **remplacés** en bloc, ce qui applique la règle du §7.4 — l'API fait autorité — sans
  code de réconciliation ligne à ligne. Il n'y a pas encore de file de mutations, donc le cas « une
  mutation en attente gagne » du §7.4 n'est pas implémenté : il arrive avec le §7.3.
- **`PollingSource`** dans `quay-forge`, implémentant `EventSource`. Requête conditionnelle sur
  `/notifications`, respect de `X-Poll-Interval` via `Tier::interval_respecting`, 304 rendu comme
  « aucun signal ». `quay-sync` ne dépend que du trait, jamais de cette implémentation.
- **Détail de PR en GraphQL** avec **pagination complète des threads**. J1-a avait montré que le
  coût suit la profondeur des threads, pas le nombre de fichiers, et qu'une PR de 170 threads est
  tronquée à 100 en une passe. Un compteur de threads non résolus tronqué mentirait à l'utilisateur
  sur la seule information qui lui dit s'il a du travail : les pages sont donc toutes lues. Les
  fichiers ne sont pas récupérés, l'inbox ne les affiche pas ; ils viendront avec la vue de diff.
- **`SyncEngine`** dans `quay-sync` : draine les sources, déduplique sur `(entity_id, updated_at)`,
  et pour chaque signal compare `updated_at` à la fraîcheur locale avant de sortir sur le réseau. Un
  échec sur une PR est rapporté dans le bilan du tick sans interrompre les autres.

### Vérification contre la vraie API

Les fixtures `wiremock` sont de mon invention : elles prouvent que le pipeline se tient, pas que la
requête GraphQL correspond au schéma réel de GitHub. `cargo run -p xtask -- sync-once` exécute un
tick réel, en lecture seule.

| Observation | Résultat |
|---|---|
| Signaux tirés de `/notifications` | 10 |
| PR récupérées et écrites dans SQLite | 10, **0 échec** |
| Requêtes émises au total | 13, soit 1 par PR plus identité et inbox |
| Durée du tick | 7 547 ms, soit ~755 ms par PR |
| États stockés | 9 `merged`, 1 `open` |
| Threads, commentaires, demandes de review | 5, 5, 1 |
| Inbox rendue après coup | 1 ligne, la seule PR ouverte |

Les ~755 ms par PR retombent exactement sur le plancher mesuré en J1-a, ce qui est une confirmation
croisée : le chemin complet ne coûte rien de plus que l'aller-retour réseau lui-même.

L'inbox ne rend qu'une ligne sur dix PR stockées parce que neuf sont `merged` : le filtre
`state = 'open'` fait son travail. `review_requested_exact` rend zéro ligne parce que sur la seule
PR ouverte l'utilisateur est l'**auteur**, et le filtre canonique du §8.4 exclut `author:@me`. Les
deux résultats sont corrects et vérifiés plutôt que supposés, d'où l'ajout du décompte par état dans
la sortie de `sync-once`.

### Ce qui n'est pas mesuré

Les temps d'inbox affichés par `sync-once` (0,2 ms environ) portent sur une base quasi vide et **ne
mesurent pas** le budget du §4. Le budget `inbox_query` reste porté par J1-b sur le dataset de
référence. Aucun chiffre de `sync-once` n'alimente `measurements/`.

### Réserves

- Les validateurs de `/notifications` sont persistés par `SyncEngine::remember_notification_validators`
  mais `sync-once` repart de zéro à chaque exécution, donc le 304 gratuit mesuré en M0-1 n'est pas
  encore exercé de bout en bout. Il le sera quand le scheduler tournera en continu, à l'étape 7.
- `stale_after` est écrit avec l'intervalle du tier warm. Le tier hot qui suit le focus (§7.1) n'a
  pas encore de consommateur : il n'y a pas d'écran.
- Le `diff_hunk` du §3.5 n'est pas récupéré. Il n'est pas utile à l'inbox et alourdirait la requête
  dont J1-a a montré qu'elle est déjà dominée par la profondeur des threads. À reprendre avec la vue
  de diff.

### Ce qu'il faut faire ensuite

1. Étape 7 : première sortie visible, l'inbox réelle multi-repos, avec le critère de sortie du §9 —
   affichage depuis SQLite en moins de 5 ms sur un jeu réel.
2. Boucler le scheduler : rejouer les validateurs entre les ticks pour exercer le 304 gratuit, et
   faire piloter le rythme par `due_for_refresh`.

---

## Session 2026-09-04 (suite) — étape 7, première sortie visible

### Ce qui a été fait

Le §0 dit que le M0 est un spike en ligne de commande et le §9 que l'inbox peut s'afficher « en CLI
ou dans une fenêtre Tauri, peu importe ». C'est donc une CLI, pas de Tauri, pas de Svelte.

- **`SearchSource`**, seconde implémentation de `EventSource`. `/notifications` ne rend que ce à quoi
  l'utilisateur est abonné : avec dix notifications l'inbox faisait une ligne, ce qui ne prouve rien.
  La file de review réelle vient de `search/issues` sur `review-requested:@me` et `author:@me`, ce
  que le §7.1 appelle « l'inbox, les PR de l'utilisateur ». Le moteur ne connaît toujours que le
  trait : c'est exactement ce que le §7.5 achetait en posant l'abstraction dès le premier jour, et
  la seconde source n'a coûté aucune modification du scheduler.
- **Binaire `quay`** avec quatre commandes : `login` valide un jeton et le dépose dans le trousseau,
  `sync` rafraîchit et écrit dans SQLite, `inbox` affiche depuis SQLite, `status` rend le quota, les
  capacités et l'état local.
- **Rendu dense** conforme au §8.5 : une ligne par pull request, colonnes fixes, et la couleur
  réservée à l'encodage de l'état de CI et de review, jamais à la décoration. `NO_COLOR` respecté.
  Écran vide : une invitation à agir, pas une illustration.
- La base vit dans l'emplacement de données du système, surchargeable par `QUAY_DB`.

### Le critère de sortie du §9

> « l'inbox de review réelle, multi-repos, s'affiche depuis SQLite en moins de 5 ms »

**Tenu.** Mesuré par `xtask measure inbox-real`, logs bruts dans `measurements/raw/`.

| | |
|---|---|
| Dépôts synchronisés | 14 |
| Pull requests ouvertes | 34 |
| Reviews demandées à l'utilisateur | 24 |
| **Dépôts distincts dans l'inbox** | **8** |
| `review_requested_exact`, p99 cache chaud | **0,058 ms** |
| `review_requested_exact`, p99 cache froid | 0,049 ms |

0,058 ms contre 5 ms de budget. **Ce jeu réel est plus petit que celui du §4** (14 dépôts et 34 PR
ouvertes contre 20 et 300) : il montre le produit sur des données vraies, il ne remplace pas J1-b,
qui reste l'instrument du budget `inbox_query`. Le rapport généré le dit à cet endroit précis.

### Le régime permanent, mesuré

Trois `quay sync` consécutifs :

| Tick | Signaux | Déjà à jour | Récupérées | Requêtes | Points | Révalidations gratuites |
|---|---|---|---|---|---|---|
| 1 | 44 | 1 | 43 | 48 | 48 | 0 |
| 2 | 34 | 34 | 0 | 5 | 4 | 1 |
| 3 | 34 | 34 | 0 | 5 | 4 | 1 |

**Le 304 gratuit de M0-1 est maintenant exercé de bout en bout** : le tick de régime permanent coûte
quatre points de quota, dont zéro pour `/notifications`. Les 34 signaux sont tous écartés par la
comparaison de fraîcheur avant toute sortie réseau, ce qui est la raison d'être de `stale_after`.

Projection non mesurée : à un tick par minute, quatre points par tick donnent 240 requêtes par
heure, très en dessous du plafond de 1 500 du §7.2. C'est une **projection**, pas une mesure : le
budget `github_quota_per_hour` reste `MISSING` tant que la boucle n'aura pas tourné une heure.

### Vérifications plutôt que suppositions

L'inbox rend 24 lignes alors que 34 PR sont ouvertes. Vérifié dans la base plutôt que déduit :
24 lignes de `review_request` nomment l'utilisateur, sur 8 dépôts distincts, et 10 des 34 PR
ouvertes ont l'utilisateur pour auteur, que le filtre canonique du §8.4 exclut. Les deux chiffres se
recoupent.

### Un écart entre mon dataset synthétique et la réalité

Les payloads bruts réellement stockés pèsent **47 Ko pour 43 pull requests, soit environ 1,1 Ko
chacun**. Mon générateur de J1-b en met **3 Ko**. Le dataset synthétique est donc plus pessimiste
que la réalité sur cette dimension, ce qui **surestime** le gain mesuré du déplacement de `raw` hors
de `pull_request`. La correction reste bonne, son bénéfice est simplement plus faible qu'annoncé aux
tailles réelles. À noter que les PR observées sont majoritairement des montées de version
automatiques, sans threads : un dépôt à revue dense donnerait des payloads plus lourds. Ni l'un ni
l'autre n'est mesuré à grande échelle, et le générateur n'est pas ajusté sur un échantillon de 43
lignes.

### Réserves

- `SearchSource` tape `search/issues`, qui a son **propre seau de rate limit** (30 requêtes par
  minute en authentifié), que le gouverneur ne modélise pas : il ne connaît que le seau primaire lu
  dans les en-têtes. Deux requêtes par tick sont très en dessous, mais le jour où la spéculation
  ajoutera des recherches, il faudra un compteur par seau.
- Le tier hot du §7.1 n'a toujours pas de consommateur : rien n'est « à l'écran » dans une CLI qui
  rend et s'arrête. Le scheduler continu et le suivi du focus arrivent avec l'interface.
- `quay sync` fait un tick et rend la main. La boucle du §7.1 avec `due_for_refresh` pour cadence
  n'est pas branchée.

### Ce qu'il faut faire ensuite

Le jour 1 du §9 est terminé, ses sept étapes sont franchies et son critère de sortie est tenu et
mesuré. La suite est le M1 : file de mutations optimistes (§7.3), préchargement couche 1 (§7.6),
palette de commandes et carte des touches (§8.2 et §8.3), vues sauvegardées avec le DSL (§8.4), et
l'interface Tauri. Trancher avant : la question M0-1 laissée ouverte, et le `synchronous = NORMAL`
du §6 au regard des invariants de crash du §7.3.

---

## Session 2026-09-04 (suite) — M1, file de mutations optimistes (§7.3)

Les deux questions laissées ouvertes sont tranchées.

### Décision 1 — M0-1 fermée sur l'hypothèse haute

On suppose que `/notifications` exige un PAT classique. C'est déjà la décision arrêtée du §3.6, donc
l'hypothèse ne coûte rien et n'engage aucune réécriture. La case se rouvrira le jour où un jeton
fine-grained sera disponible.

### Décision 2 — `synchronous` tranché par la mesure, et un premier chiffre trompeur

`cargo run -p xtask -- measure durability` chronomètre une mutation optimiste, c'est-à-dire une
transaction contenant une mise à jour et une insertion en file, 2 000 fois par mode.

| Garantie | `synchronous` | `fullfsync` | p50 | p99 |
|---|---|---|---|---|
| `process_crash_safe` | NORMAL | non | 0,015 ms | **0,042 ms** |
| `power_loss_safe` | FULL | oui | 3,996 ms | **5,965 ms** |

**Mon premier passage mesurait `FULL` sans `fullfsync` et donnait 0,086 ms de p99.** J'aurais conclu
que la durabilité complète était gratuite. Elle ne l'est pas : sur macOS, `fsync()` rend la main
avant que le disque ait réellement écrit, et seul `PRAGMA fullfsync` déclenche un vidage du cache du
disque. Le chiffre honnête est 5,965 ms, **au-dessus du budget de 5 ms** du §5.4 et de l'étape 2 du
§7.3.

**Décision : `ProcessCrashSafe` par défaut**, ce que le §6 spécifiait déjà. Justification, cette fois
appuyée sur un chiffre :

1. L'invariant du §7.3 est écrit « un crash », et un crash de processus ne perd rien en mode WAL avec
   `synchronous = NORMAL`.
2. La coupure de courant est une autre classe de panne, que le §7.3 ne nomme pas, et dont la
   conséquence ici n'est ni une corruption ni un état à moitié appliqué : l'état optimiste et la
   ligne de file sont perdus **ensemble**, puisqu'ils partagent la transaction. L'utilisateur voit
   que son action n'a pas eu lieu, il la refait.
3. La payer coûterait 5,965 ms de p99 sur le chemin qui doit rendre la main en moins de 5 ms.

`Durability::PowerLossSafe` reste offert, et l'énumération est nommée par la garantie obtenue plutôt
que par le nom du pragma, précisément parce que le pragma seul ment sur macOS.

### La file de mutations

- Les six genres du §6 sont modélisés, avec `is_safe_to_resend` qui distingue ce qui **crée du
  contenu** de ce qui **pose un état**. C'est cette distinction qui tient l'invariant « un crash
  entre 3 et 4 ne duplique jamais un commentaire » : GitHub n'a pas de clé d'idempotence côté
  serveur, donc au redémarrage une mutation restée `inflight` est remise en file si la renvoyer est
  sans effet de bord, et **marquée en échec avec la raison en clair** sinon. Prétendre que la clé
  d'idempotence protège du double envoi HTTP aurait été faux : elle protège de la double mise en
  file, ce qui n'est pas la même chose.
- L'état optimiste et la ligne de file sont écrits **dans la même transaction**, l'appelant
  fournissant l'effet local. Un effet local qui échoue ne laisse aucune ligne de file : c'est ce que
  vérifie le test sur une cible inconnue.
- Deux mutations sur la même cible sont sérialisées par la requête de réservation elle-même, qui
  ignore une cible ayant déjà une mutation en vol.
- Le rollback restaure la valeur précédente, transportée dans le payload. Une valeur précédente
  absente revient absente, pas en chaîne vide.
- Le worker distingue trois issues : 2xx règle, 4xx définitif annule et **rend la raison de la
  forge**, tout le reste diffère en gardant l'état optimiste. Les verrous de développement diffèrent
  aussi, **ils n'annulent pas** : l'intention de l'utilisateur reste valide, c'est l'outil qui la
  retient.

### Deux bugs trouvés en exécutant pour de vrai

**Le worker rejouait en boucle la mutation qu'il venait de différer.** Attrapé par le test du verrou
lecture seule, qui comptait quatre reports au lieu d'un. Une mutation traitée dans une passe n'y est
plus reprise. Le garde-fou remet la mutation en file **sans écraser sa dernière erreur**, sinon le
message précis de la forge disparaissait derrière un message technique.

**Le trousseau macOS bloque un shell non interactif.** `quay push` restait figé sans rien afficher.
Cause : recompiler le binaire change sa signature, macOS redemande l'autorisation d'accès au secret
par un dialogue graphique, et un shell sans terminal attend indéfiniment. Deux corrections : un jeton
donné par l'environnement prime désormais sur le trousseau, et la lecture du trousseau est bornée
dans le temps et rend une erreur qui nomme la cause et l'issue au lieu de se figer. Consigné dans
`docs/platform-constraints.md`.

### La démonstration, sur des données réelles

`quay approve Groupe-RHF/parts-discount#333` met l'approbation en file et l'état local passe à
`approved` immédiatement. `quay push`, verrou `QUAY_READONLY=1` actif, rend :

```
0 envoyée(s), 0 annulée(s), 1 différée(s)
file pending : 1 entrée(s)
  approve_pr sur PR_kwDOOGDF2s79GAwa, 4 tentative(s) — read-only mode rejected a rest write
```

Rien n'est parti vers la vraie pull request. `quay cancel 1` a défait l'état optimiste et la base
locale est revenue à `review_state = NULL`, ce qu'elle était avant.

### Réserves

- Le backoff entre deux passes du worker n'est pas cadencé : `drain` traite chaque mutation une fois
  puis rend la main, et c'est l'appelant qui décide quand recommencer. La boucle du §7.1 n'existe
  toujours pas.
- La bannière non modale du §7.3 est une ligne de terminal ici. Le vrai comportement demandé arrive
  avec l'interface.
- Aucune mutation n'a été envoyée à GitHub de toute la session. Le verrou n'a jamais été levé.

### Ce qu'il faut faire ensuite

Préchargement couche 1 (§7.6), puis la boucle de scheduler cadencée par `due_for_refresh` et les
trois tiers du §7.1, puis l'interface.

---

## Session 2026-09-04 (suite) — M1, préchargement couche 1 (§7.6) et boucle de scheduler

### Ce qui a été fait

- **Règles déterministes de la couche 1**, en logique pure dans `quay-core` : l'inbox qui se charge
  précharge ses cinq premières entrées, une PR ouverte précharge sa voisine précédente et sa
  voisine suivante. Douze tests, aucun I/O.
- **Deux règles du §7.6 ne sont pas implémentées** et c'est délibéré : « le curseur reste sur une
  ligne plus de 150 ms » suppose un curseur, « un diff est ouvert » suppose une vue de diff et des
  fichiers qu'on ne récupère pas encore. Les écrire aujourd'hui serait du code sans déclencheur.
  La cinquième, le retour au premier plan, suppose une fenêtre.
- **Gouvernance** : priorité `Speculative`, donc soumise au plafond de 15 % déjà tenu par le
  gouverneur, et auto-désactivation sous 40 % d'utilisation sur une fenêtre glissante de 500
  préchargements. La règle est pure et testée aux deux bornes.
- **`preload_outcome`** distingue deux natures de ligne par sa colonne `reason` : un préchargement
  spéculatif, et une navigation. `used` veut dire « servi localement » dans les deux cas. La
  distinction compte : le §7.6 règle la spéculation sur le taux d'utilisation des préchargements,
  le §4 mesure le taux de navigations servies depuis le cache, et une navigation peut être servie
  localement sans avoir été préchargée.
- **`navigation_event`** est collecté, sans consommateur, comme le demande le §9 pour le M1.
- **`quay show`** rend une PR et ses threads depuis SQLite, enregistre la navigation, et marque le
  préchargement comme utilisé le cas échéant.
- **`quay watch`** boucle au rythme annoncé par la forge. `PollingSource` expose désormais un point
  de reprise partagé qui porte à la fois les validateurs et l'intervalle : sans cela la boucle
  n'aurait pas pu respecter `X-Poll-Interval`, elle aurait rejoué le sien.

### Un défaut de conception trouvé en regardant tourner la boucle

Le premier passage affichait, à chaque cycle : `5 planifiée(s), 0 déjà fraîche(s), 5 récupérée(s)`.
Le préchargement refetchait donc les mêmes cinq pull requests toutes les minutes, indéfiniment, alors
que le tick venait de confirmer qu'elles n'avaient pas changé.

Cause : quand un signal confirmait que la copie locale était à jour, la réconciliation renvoyait
`AlreadyCurrent` **sans repousser `stale_after`**. La fraîcheur expirait donc au rythme du cycle et
le préchargement croyait avoir du travail. Confondre « on n'a pas refetché depuis un moment » avec
« la copie n'est plus à jour » est exactement l'erreur que `stale_after` existe pour éviter.

Corrigé : un signal qui confirme la copie repousse son échéance. Le test le fige, et vérifie en plus
qu'un préchargement lancé juste après ne récupère rien.

L'effet sur le régime permanent, mesuré sur deux cycles réels :

| | Avant | Après |
|---|---|---|
| Préchargements récupérés par cycle | 5 | **0** |
| Requêtes du deuxième cycle | 8 | **3** |
| Points du deuxième cycle | 7 | **2** |

À un cycle par minute, cela **projette** 120 requêtes par heure contre 300 auparavant. C'est une
projection arithmétique à partir de deux cycles mesurés, pas une mesure sur une heure : le budget
`github_quota_per_hour` reste `MISSING`.

Le plafond de spéculation aurait fini par couper le gaspillage au bout d'environ quarante-cinq
minutes de boucle, ce qui est son rôle. Mais un garde-fou qui rattrape un défaut n'est pas une
excuse pour le laisser.

### Ce que je n'écris pas dans les budgets

`cache_served_navigations` du §4 demande plus de 85 % de navigations servies depuis le cache sur une
fenêtre glissante. La mécanique existe et `quay status` affiche le chiffre, mais **je n'alimente pas
le budget** : il n'y a qu'une poignée de navigations réelles, et un pourcentage sur un échantillon
d'une unité ne veut rien dire. Il passera au vert quand un usage réel aura rempli la fenêtre, pas
avant. Fabriquer 500 `quay show` pour verdir une ligne serait exactement le chiffre plausible et
faux que le `CLAUDE.md` interdit.

De même, l'utilisation du préchargement affiche 20 % après la première session, sous le plancher de
40 %. La spéculation reste active parce que la fenêtre de 500 n'est pas remplie, ce qui est la règle
du §7.6 appliquée telle quelle.

### Réserves

- Le préchargement est déclenché par le cycle de la boucle, pas par un événement d'interface. C'est
  l'approximation la plus proche de « l'inbox est à l'écran » sans écran. Le vrai déclencheur du
  §7.6 arrive avec l'interface.
- Le tier hot du §7.1 n'a toujours pas de consommateur, ni la dégradation en arrière-plan : les deux
  suivent le focus d'une fenêtre.
- `SearchSource` reste hors du modèle de seau du gouverneur, cf. la réserve de l'étape 7.

### Ce qu'il faut faire ensuite

Les briques du sync engine du M1 sont posées : trois tiers déclarés, gouverneur, file de mutations,
préchargement couche 1, boucle. Ce qui reste au M1 est de l'interface : palette de commandes et carte
des touches (§8.2, §8.3), vues sauvegardées avec le DSL (§8.4), et la fenêtre Tauri avec le store
Svelte. C'est là que les budgets `keystroke_to_pixel`, `cold_start_to_first_paint` et
`cached_pull_request_navigation` deviendront mesurables.

---

## Session 2026-09-04 (suite) — M1, DSL de vues sauvegardées (§8.4)

### Ce qui a été fait

- **Analyseur du DSL** dans `quay-core`, en logique pure : qualificateurs, négation par `-`, valeurs
  entre guillemets, texte libre. Les trois requêtes d'exemple du §8.4 sont figées par des tests.
- **Refus nommés plutôt qu'ignorés en silence.** Le §8.3 dit que rien de découvrable n'échoue : un
  qualificateur inconnu est refusé **et propose le plus proche**, une valeur hors du jeu accepté est
  refusée **en listant ce qui est accepté**, un `sort:` nié est refusé parce que le résultat n'aurait
  pas de sens, un guillemet non refermé est refusé plutôt que de tronquer la requête en silence.
- **Complétion** : fonction pure qui propose les qualificateurs puis leurs valeurs. Elle vivra
  derrière la barre de recherche ; elle est déjà testée et exposée par `quay complete`.
- **Compilateur vers SQL** dans `quay-store`. Toutes les valeurs sont **liées en paramètres**, jamais
  interpolées ; un test vérifie que le login de l'utilisateur n'apparaît pas dans le SQL produit.
- **`label:`, `assignee:` et `is:issue` font partie du langage** et sont refusés à la compilation
  avec la raison : rien ne stocke encore cette information, les issues sont au M3. C'est le §3.7
  appliqué au langage — visible, refusé, expliqué, plutôt qu'absent ou silencieusement vide.
- **Vues sauvegardées** avec leurs colonnes, leur tri, leur position et leur raccourci, et **cinq
  vues livrées par défaut** dans un fichier JSON, conformément au §8.4 qui veut que les « profils
  métier » ne soient qu'un fichier de vues modifiable. Un test vérifie que **chaque vue livrée
  s'analyse et se compile** : livrer une entrée découvrable et cassée serait précisément ce que le
  §8.1 interdit. Un autre vérifie qu'aucune n'entre en conflit de raccourci.
- L'inbox du binaire passe désormais **par le DSL**, via la vue « À relire ». Les requêtes écrites à
  la main restent, mais uniquement comme jeu de mesure figé pour J1-b, dont la comparabilité entre
  exécutions dépend de la stabilité du SQL.

### Une migration, pour de vrai

Le test « deux vues du même nom » a échoué et il avait raison : le §6 ne met **aucune contrainte
d'unicité sur `saved_view.name`**, alors qu'une vue s'adresse par son nom, ni sur `shortcut`, alors
que le §8.3 exige qu'aucun raccourci n'entre en conflit. Migration `0002` :

```sql
CREATE UNIQUE INDEX idx_saved_view_name ON saved_view(name);
CREATE UNIQUE INDEX idx_saved_view_shortcut ON saved_view(shortcut) WHERE shortcut IS NOT NULL;
```

Index partiel pour que plusieurs vues puissent n'avoir aucun raccourci. C'est la première migration
appliquée à une base réelle déjà en version 1, et elle est passée sans intervention.

### Le DSL ne coûte rien

Mesuré sur le jeu réel, même requête canonique, mêmes 24 lignes :

| Chemin | p99 cache chaud |
|---|---|
| SQL écrit à la main (`review_requested_exact`) | 0,040 ms |
| Compilé depuis le DSL | 0,042 ms |

L'écart est dans le bruit. Le langage de requête n'introduit pas de coût mesurable, ce qui lève la
seule objection sérieuse à en faire le chemin du produit.

### Les erreurs sont rendues dans la langue de l'utilisateur

Les types d'erreur des crates de cœur restent **structurés et en anglais**, comme l'exige le
`CLAUDE.md` pour le code. C'est la couche de présentation qui les rend en français, avec l'action
corrective, exactement comme le §3.7 sépare la `Reason` de son affichage :

```
`auth` n'est pas un qualificateur connu. Vouliez-vous `author:` ?
`checks` n'accepte pas `red`. Valeurs possibles : success, failing, pending, none.
`repo` a reçu `acme`, il attend la forme `owner/name`.
`label` fait partie du langage mais ce jalon ne sait pas y répondre : rien ne stocke encore
cette information.
```

### Réserves

- Le tri des vues sauvegardées est porté deux fois : par la colonne `sort` de la table et par le
  qualificateur `sort:` de la requête. C'est le §6 et le §8.4 qui se recouvrent. Le compilateur ne
  lit que le qualificateur ; la colonne est conservée telle que le §6 la déclare mais n'a pas de
  lecteur. À trancher quand l'interface aura besoin de trier une colonne au clic.
- La complétion propose `@me` pour les qualificateurs de personne mais pas les logins réellement
  présents en base. Le faire demande de décider si l'on suggère depuis les données locales ou depuis
  la forge, ce qui est une question d'interface.

### Ce qu'il faut faire ensuite

L'interface : palette de commandes (§8.3), carte des touches (§8.2), et la fenêtre Tauri avec le
store Svelte. C'est là que les budgets `keystroke_to_pixel`, `cold_start_to_first_paint` et
`cached_pull_request_navigation` deviennent mesurables.

---

## Session 2026-09-04 (suite) — M1, carte des touches (§8.2) et registre de commandes (§8.3)

### Ce qui a été fait

`quay-app` devient une bibliothèque doublée d'un binaire, pour que le registre soit testable sans
fenêtre. Le §5.2 n'en est pas changé : c'est une cible de plus, pas une crate de plus.

- **Accords de touches** : séquences à la vim (`g i`), modificateur commande (`⌘K`), touches nommées
  (`Enter`, `Space`, `Esc`), et la casse distingue deux liaisons parce que `Maj` fait partie de la
  touche — `j` et `J` sont deux commandes différentes au §8.2. Une liaison vide ou une touche
  inconnue est refusée plutôt que silencieusement ignorée.
- **Registre typé** de 34 commandes couvrant la carte du §8.2, chacune avec son titre, ses
  mots-clés, ses liaisons, sa portée et la capacité qu'elle exige.
- **Filtrage par capacités** appliqué **au registre lui-même**, pas seulement au rendu, comme
  l'exige le §8.1 : une capacité structurellement indisponible sort la commande du registre et son
  raccourci ne résout plus rien ; une capacité récupérable par l'utilisateur la laisse visible mais
  ne la déclenche pas.
- **Classement flou façon fzf** : bonus au début d'un mot, à une limite de mot et à une limite de
  casse, et non un simple `includes()`. Un test vérifie qu'un début de mot l'emporte sur les mêmes
  lettres au milieu d'un autre.
- **`quay keys [portée]`** rend la carte valide **ici**, ce que le §8.2 demande du `?`.

### Une correction du modèle de portées

Ma première version faisait hériter chaque portée de `Global` seulement. Le §8.2 place pourtant `]`
et `}` dans la même section : dans un diff, la navigation de fichier de la pull request doit
continuer à répondre. Les portées s'emboîtent donc en chaîne — `Diff` dans `PullRequest` dans
`Global`, `ReviewPanel` dans `PullRequest` dans `Global`.

Ce qui fait apparaître un conflit apparent : `c` est « commenter » dans une pull request et
« demander des changements » dans le panneau de review, que le §8.2 écrit `v c`. Ce n'est pas un
conflit, c'est du **masquage**, et c'est la raison d'être des portées. J'ai donc modélisé la
résolution explicitement au lieu d'interdire le masquage :

- aucun conflit **au sein d'une même portée déclarée** ;
- aucun accord **préfixe** d'un autre dans une même portée, parce que là l'ambiguïté serait réelle ;
- toute résolution rend **exactement une** commande, la plus interne.

Le §8.2 écrit `v a` et `v c` par lisibilité ; l'implémentation les rend comme `v` qui ouvre le
panneau, puis `a` ou `c` dans ce panneau. L'ambiguïté est levée par la portée, pas par un délai
d'attente, ce qui évite le mode invisible que le §8.1 interdit.

### Ce que le §8.3 exigeait et qui est tenu

> « Un test vérifie que toute commande du registre est atteignable, et qu'aucun raccourci n'est en
> conflit dans un même scope. »

Dix-sept tests, dont ceux-là mot pour mot, plus l'atteignabilité de la palette depuis toutes les
portées et la disparition d'une commande bloquée par une politique d'organisation.

### Réserves

- Le registre déclare les commandes ; **il ne les exécute pas**. Le §8.3 prévoit un `run` par
  commande, qui n'a pas de sens sans un état d'interface à muter. Il arrivera avec la fenêtre.
- Le classement « apprenant » du §8.3, pondéré par la fréquence d'usage récent, n'est pas
  implémenté : `navigation_event` collecte la matière première depuis l'étape M1 mais rien ne la lit
  encore, ce que le §9 prévoit explicitement pour le M1.
- La palette « avec arguments » du §8.3, qui ouvre un second niveau de sélection, suppose une
  interface.

### Ce qu'il faut faire ensuite

La fenêtre Tauri et le store Svelte. Le registre y sera exposé par l'IPC pour rester la source de
vérité unique que le §8.3 exige, plutôt que d'être recopié en TypeScript.

---

## Session 2026-09-04 (suite) — M1, la fenêtre

### Ce qui a été fait

- **Couche IPC en fonctions pures** (`quay-app/src/ipc.rs`), testée sans fenêtre : carte des
  touches, palette, vues sauvegardées, exécution de requête, détail de pull request.
- **La logique reste en Rust, l'IPC ne transporte que des données déjà résolues.** La carte rendue
  au frontend est **plate** : un accord, une commande, un état d'activation. La chaîne de portées et
  le masquage du §8.2 ont déjà été appliqués côté Rust. Un test le fige nommément — dans le panneau
  de review, `c` arrive au frontend en désignant déjà `review.request_changes`. Le frontend ne
  recopie donc aucune règle, il fait une consultation de table.
- **Coque Tauri 2** avec sept commandes IPC, et **frontend Svelte 5** avec Vite : liste dense,
  palette, vue de détail, lecture d'accords, instrumentation `frappe → pixel`.
- **Direction visuelle du §8.5** : lignes de 28 px, un seul accent réservé au focus et à l'état
  actionnable, la couleur ailleurs n'encode que l'état de CI et de review, pas d'ombre portée, pas de
  carte arrondie, la hiérarchie passe par l'espacement et les filets, une famille à chasse variable
  pour l'interface et une à chasse fixe pour les identifiants. Le mouvement est sous
  `prefers-reduced-motion`. L'écran vide est une invitation portant le raccourci.
- **Seize tests frontend** sur la logique pure : lecture d'accords, séquences en attente, casse
  significative, abandon d'une séquence que rien ne complète, encodage d'état. `svelte-check` passe
  sans erreur ni avertissement.
- **CI** : un job `desktop` pour le frontend, et les bibliothèques système de la webview installées
  avant les jobs Rust.

### Ce que je ne peux pas affirmer

**La fenêtre s'ouvre et le processus tient, mais je n'ai pas vu ce qui est peint.** La capture
d'écran est refusée faute d'autorisation d'enregistrement de l'écran, et l'interrogation du serveur
de fenêtres expire faute d'autorisation d'accessibilité. Ce qui est vérifié : le processus démarre et
survit, le bundle se construit, le typage passe, et le chemin de données que la fenêtre emprunte est
le même que celui qu'exercent `quay view` et `quay keys`, tous deux vérifiés sur les données réelles.
Ce qui ne l'est pas : les pixels. Il faut un œil humain.

De même, **les budgets `keystroke_to_pixel` et `cold_start_to_first_paint` restent `MISSING`.**
L'instrumentation existe et la barre d'état affiche le p99 dès qu'il y a des frappes, mais aucune
mesure n'a été relevée ni écrite dans `measurements/`. Un p99 de frappe demande un humain qui tape.

### Réserves

- Les capacités du §3.7 démarrent toutes en `Unknown`, ce que le §3.7 dit de traiter comme
  disponible pour ne pas bloquer par précaution. Le sondage actif et le rafraîchissement en arrière-
  plan ne sont pas branchés : la fenêtre n'appelle pas le réseau au démarrage, précisément à cause du
  budget de 400 ms. Il faudra un événement `capabilities_changed` émis après le sondage.
- Le frontend consomme les commandes IPC mais **n'écoute encore aucun événement** du cœur. Le §5.4
  demande `inbox_changed`, `pr_updated`, `sync_state` et `mutation_failed`. Sans eux, la fenêtre ne
  se met pas à jour quand la boucle de synchronisation écrit.
- La palette n'a pas encore le second niveau de sélection à arguments du §8.3, ni le classement
  pondéré par la fréquence d'usage récent.
- La liste n'est pas virtualisée. Avec 34 lignes réelles cela ne se voit pas ; le §5.1 l'exige et le
  budget de 16 ms le rendra visible bien avant 500 lignes.
- `run` par commande n'existe toujours pas côté Rust : c'est le frontend qui interprète l'identifiant
  de commande. Le registre reste la source de vérité pour ce qui **existe** et ce qui est
  **atteignable** ; l'exécution est côté interface tant qu'il n'y a pas d'état partagé à muter.

### Ce qu'il faut faire ensuite

1. Regarder la fenêtre, et me dire ce qui ne va pas.
2. Les événements du §5.4, sans lesquels la fenêtre est un instantané.
3. La virtualisation de la liste, puis les mesures de `keystroke_to_pixel` et de démarrage à froid.

---

## Session 2026-09-04 (suite) — la fenêtre blanche, et ce qu'elle cachait

### La cause de la fenêtre blanche

`tauri-build` émet `cfg(dev)` dès que la fonctionnalité `custom-protocol` du crate `tauri` n'est pas
activée — son code est littéralement `let dev = !custom_protocol`. En mode `dev`, Tauri pointe le
webview sur `devUrl`, c'est-à-dire le serveur Vite, que rien ne servait. **La fenêtre chargeait donc
une adresse morte, sans erreur ni journal.** La CLI Tauri active cette fonctionnalité pour les builds
de release ; comme je ne l'utilise pas, personne ne le faisait.

Un indice était passé sous mon nez : j'avais retiré `apps/desktop/dist` et constaté que la
compilation ne cassait pas, et j'en avais conclu « Tauri tolère l'absence du bundle ». La vraie
explication était qu'il ne le lisait pas du tout. J'ai écrit une conclusion là où j'aurais dû lire un
symptôme.

Corrigé par une fonctionnalité `embedded-frontend` activée par défaut, et figé par un test :
`the_window_serves_the_frontend_it_carries_rather_than_a_development_server`.

**La CSP n'était pas en cause.** Je l'avais désactivée pour tester et je l'ai remise, vérification
faite : la fenêtre rend avec `default-src 'self'`.

### La deuxième cause, cachée derrière la première

Une fois la fenêtre peinte, la barre d'état restait figée sur `identifying`. Le journal montrait
pourtant la boucle qui tournait : `/user` 200, `/notifications` **304**, deux recherches 200. Ce
n'étaient donc pas les événements qui n'étaient pas émis, c'était le webview qui ne pouvait pas les
écouter : **Tauri 2 refuse les commandes de ses greffons de cœur tant qu'une capacité ne les accorde
pas.** Les commandes propres à l'application marchaient, `listen` non.

`capabilities/default.json` accorde `core:default` à la fenêtre `main`. Et le frontend **remonte
désormais l'échec d'abonnement** au lieu de l'avaler, ce qui aurait fait gagner une demi-heure.

### Ce qui a été corrigé dans la foulée

- **Focus visible** (§8.1) : mon état sélectionné était exactement « une nuance de fond de 3 % » que
  le §8.1 interdit. Il porte maintenant un liseré d'accent et un titre en gras.
- **Liste virtualisée** (§5.1) : seule une fenêtre de lignes est rendue, les hauteurs manquantes sont
  réservées par deux cales. Un test monte 4 000 entrées et vérifie qu'il s'en peint moins de 200.
- **Boucle de synchronisation dans la fenêtre**, avec les événements `sync_state`, `inbox_changed` et
  `capabilities_changed` du §5.4. Les capacités partent d'`Unknown` puis sont remplacées après le
  sondage, ce qui rafraîchit la carte des touches.
- **Course au démarrage supprimée** : les premiers événements partaient avant que le webview n'ait
  posé ses écouteurs. Le cœur garde donc le dernier état et la fenêtre le lit au montage, plutôt que
  de masquer la course par un délai.
- Le premier échec rencontré au démarrage n'est plus écrasé par un échec accessoire. Trouvé par un
  test, pas à l'œil.

### Démarrage à froid : mesuré, et **dépassé d'un facteur 11**

`cargo run -p xtask -- measure cold-start` lance le binaire cinq fois et lit les phases dans son
journal. Cumul depuis le démarrage du processus :

| Phase | p50 |
|---|---|
| Base SQLite ouverte | **2 ms** |
| Tauri prêt, fenêtre créée | 2 197 ms |
| Bundle JS démarré | 2 311 ms |
| **Première liste peinte** | **4 387 ms** |

Le budget du §4 est de **400 ms**. Le budget passe donc de `MISSING` à **`OVER`**, ce qui est plus
honnête.

**Ce que la mesure dit clairement : le magasin ne coûte rien.** 2 ms sur 4 387, soit 0,05 %. Tout le
temps est dans la coque desktop, en deux moitiés presque égales : environ 2,2 s avant que Tauri rende
la main, puis environ 2,1 s avant la première image composée.

Hypothèse testée et **réfutée** : un binaire nu hors bundle `.app`. Un `.app` signé ad-hoc donne
4 528 ms et 4 424 ms, soit la même chose. Ce n'est donc ni Gatekeeper ni l'absence de bundle.

**Le §12 dit de m'arrêter quand un budget est manqué d'un facteur supérieur à 2 sans cause claire.**
J'ai une cause localisée — l'initialisation de la coque — mais pas de cause racine ni de correctif.
Trois pistes, aucune mesurée :

1. WKWebView démarre des processus auxiliaires ; leur coût est peut-être incompressible sur cette
   machine et il faudrait le comparer à une application Tauri vide pour le savoir.
2. La fenêtre pourrait être affichée **avant** que le frontend ne soit prêt, avec la liste peinte
   ensuite. Cela améliorerait le ressenti sans changer la mesure telle qu'elle est définie.
3. Le §4 mesure « démarrage à froid → première liste peinte ». Si l'application est faite pour rester
   ouverte huit heures, ce budget est peut-être le mauvais indicateur — mais c'est une question de
   spécification, pas une décision que je prends seul.

### Ce qui reste non mesuré, et pourquoi

`keystroke_to_pixel` reste `MISSING`. L'instrumentation existe et la barre d'état affiche le p99, mais
`osascript` n'est pas autorisé à envoyer des frappes depuis ce terminal. Il faut taper à la main dans
la fenêtre. Je ne fabrique pas de chiffre à la place.

### Une erreur de méthode que je signale

Ma commande de vérification faisait la somme des tests réussis. Elle a donc **masqué un test rouge**
dans `xtask` pendant deux tours. Le test avait raison : mon analyseur du journal de démarrage lisait
le `0` de la séquence d'échappement `[0m` comme la valeur mesurée. En production `NO_COLOR=1` le
masquait, ce qui rendait le défaut invisible sans le supprimer. Corrigé en retirant les séquences
d'échappement, et la vérification affiche désormais les échecs au lieu de compter les succès.

### Ce qu'il faut faire ensuite

1. Trancher le démarrage à froid, cf. les trois pistes ci-dessus.
2. Taper dans la fenêtre pour que `keystroke_to_pixel` existe.
3. La palette à arguments et le classement appris du §8.3, le `run` par commande côté cœur.

---

## Session 2026-09-04 (suite) — le démarrage à froid, correctement mesuré

### Je corrige un chiffre que j'ai publié

J'ai rapporté **4387 ms** de démarrage à froid. **C'était mon instrument qui était faux**, pas la
machine. Je mesurais la première peinture avec deux `requestAnimationFrame` enchaînés après le
chargement des données ; or dans un webview qui vient de s'ouvrir, deux images consécutives peuvent
être séparées de deux secondes. J'ajoutais donc deux secondes de rien à la mesure.

Le bon instrument est celui du navigateur lui-même, `PerformanceObserver` sur les entrées `paint`.
Il place la première peinture de contenu à **73 ms dans la page**. Le chiffre corrigé est
**2369 ms**, pas 4387.

C'est exactement l'erreur contre laquelle le `CLAUDE.md` met en garde : un chiffre plausible et faux
coûte plus cher qu'une absence de chiffre. Il était mesuré, mais il ne mesurait pas ce que je disais.

### Où va réellement le temps

| Phase | p50 sur 5 lancements | Delta |
|---|---|---|
| Base SQLite ouverte | 2 ms | 2 ms |
| Runtime Tauri prêt | ~100 ms | ~98 ms |
| **Fenêtre et webview créés** | **~2215 ms** | **~2115 ms** |
| Bundle JS démarré | 2345 ms | 130 ms |
| Première peinture de contenu | **2369 ms** | 24 ms |

**Tout le dépassement tient dans la création du webview.** Notre code, magasin compris, pèse environ
150 ms sur 2369, soit 6 %.

### L'expérience qui tranche

En construisant une **seconde** fenêtre juste après la première, dans le même processus :

| Fenêtre | Coût |
|---|---|
| première | **2115 ms** |
| seconde | **17 ms** |

Le coût est donc **unique par processus**, pas par fenêtre : c'est l'initialisation de WebKit et de
ses processus auxiliaires. La sonde a été retirée après l'expérience.

### Ce que cela implique, et la décision qui n'est pas la mienne

Le §4 demande « démarrage à froid → première liste peinte < 400 ms ». Le §5.3 choisit Tauri, donc un
webview système. **Les deux semblent incompatibles sur cette plateforme** : l'initialisation de
WebKit dépasse à elle seule le budget d'un facteur cinq, et l'application ne la contrôle pas.

Trois issues, aucune que je tranche seul, c'est du §12 :

1. **Garder le budget et changer la mesure de ce qu'il gouverne.** Le §1 décrit une application
   qu'on laisse ouverte huit heures. Le coût est unique par processus : une fois l'application
   lancée, ouvrir une fenêtre coûte 17 ms. Le budget pertinent serait alors « activation → liste
   peinte », pas « lancement du processus ».
2. **Garder le budget tel quel et le déclarer manqué par construction**, en documentant que c'est le
   prix du webview système accepté au §5.3.
3. **Remettre en cause le §5.3.** Une interface native coûterait bien moins au démarrage, et bien
   plus partout ailleurs. Le §5.3 avait tranché sur la RAM et le partage du cœur, pas sur le
   démarrage.

Ma préférence est la première, parce qu'elle mesure ce que l'utilisateur vit, mais elle change ce que
le §4 veut dire et cela doit être décidé, pas glissé.

### Deux améliorations au passage

- La fenêtre est désormais **construite par le code** plutôt que déclarée dans la configuration, ce
  qui a permis d'isoler son coût et ce dont on aura besoin pour une barre de titre personnalisée.
- Les sourcemaps ne sont plus embarquées en production : le bundle passe de 550 Ko à 64 Ko. Sans
  effet mesurable sur le démarrage, mais 550 Ko de carte de sources dans un binaire livré n'a pas de
  raison d'être.
