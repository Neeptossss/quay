# CLAUDE.md — règles opératoires

Ce fichier est le contrat d'exécution. `QUAY-SPEC.md` dit **quoi** construire, ce fichier dit **comment travailler**. En cas de contradiction entre les deux, c'est la spec qui gagne sur le fond et ce fichier qui gagne sur la méthode.

---

## Le projet en trois phrases

Client desktop GitHub, local-first, clavier-first, centré sur la revue de pull requests multi-repos. Cœur Rust, interface Tauri 2 + Svelte 5. Le produit se juge à la latence perçue, pas au nombre de fonctionnalités.

Lis `QUAY-SPEC.md` en entier avant la première ligne de code. Relis le §4 (budgets) et le §7 (sync engine) avant chaque session.

Les extraits de code de la spec sont **illustratifs**. Ils contiennent des commentaires explicatifs qui n'ont pas leur place dans le dépôt : reprends l'intention, jamais les commentaires.

---

## Ordre de travail

L'ordre du §10 de la spec est strict. Ne saute pas d'étape, même si une étape ultérieure paraît plus intéressante ou plus rapide à montrer.

Interdits tant que l'étape 7 n'est pas atteinte :
- toute fenêtre Tauri, tout composant Svelte, toute maquette
- toute vue de diff
- toute file de mutations
- toute palette de commandes

Ces choses sont dans la spec parce qu'elles arrivent, pas parce qu'elles arrivent maintenant.

---

## Définition de « terminé »

Une tâche est terminée quand **toutes** ces conditions sont vraies :

1. `cargo fmt --check` passe.
2. `cargo clippy --all-targets -- -D warnings` passe.
3. `cargo test --workspace` passe.
4. Les benchs concernés sont dans leur budget, ou l'écart est documenté avec une hypothèse de cause.
5. Les chemins d'erreur sont testés, pas seulement le chemin heureux.
6. Aucun `TODO` laissé sans issue correspondante.
7. Aucun commentaire dans le code ajouté.

Pas de commit avec un bench rouge sans mention explicite dans le message de commit et une ligne dans `NOTES.md`.

---

## Commandes

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo bench -p quay-store          # budget inbox < 5 ms
cargo run -p xtask -- latency-log  # logger de latence, tourne en fond
```

Le token de test est lu depuis la variable d'environnement `QUAY_TEST_TOKEN`. **Ne jamais l'écrire dans un fichier, un test, un log ou un message de commit.** Si tu as besoin d'un token et qu'il est absent, arrête-toi et demande.

---

## Règles de code non négociables

- `#![deny(clippy::unwrap_used, clippy::expect_used)]` dans `quay-forge`, `quay-store`, `quay-sync`. Un `unwrap` sur une réponse réseau est un crash chez l'utilisateur.
- Le graphe de dépendances entre crates du §5.2 est vérifié par un test. Ne le contourne pas avec une dépendance « temporaire ».
- Une réponse d'API est désérialisée dans un type de la couche transport, jamais directement dans un type du domaine.
- Aucun secret ne franchit la frontière IPC vers le frontend.
- Toute requête sortante passe par le rate governor. Aucune exception, y compris dans les tests d'intégration et les scripts de mesure.
- Journalisation via `tracing`, avec redaction des tokens.

---

## Mesures et honnêteté

**Un chiffre non mesuré ne s'écrit pas.**

- Toute valeur de latence, de débit ou de quota qui apparaît dans un rapport, un commentaire ou un message doit provenir d'une exécution réelle dont les logs bruts sont commités dans `measurements/`.
- Les rapports sont **générés par un script** à partir de ces logs, jamais rédigés à la main.
- Une estimation est autorisée si elle est étiquetée « estimation, non mesurée ». Ne présente jamais une estimation comme une mesure.
- Si une mesure contredit une hypothèse de la spec, ne l'ajuste pas pour qu'elle passe. Signale la contradiction.

C'est la règle la plus importante de ce fichier. Un chiffre plausible et faux coûte plus cher qu'une absence de chiffre.

---

## Chemins d'erreur d'abord

Dans ce produit, les chemins d'erreur *sont* le produit. Pour tout code réseau, écris les tests d'échec avant les tests de succès :

- 403 et 429, avec et sans `Retry-After`
- 304 avec et sans ETag en cache
- réponse tronquée, JSON invalide, timeout
- token révoqué en cours de session
- org sous SSO non autorisée
- quota épuisé

Le serveur simulé (`wiremock`) doit savoir produire chacun de ces cas.

---

## Quand tu es bloqué

Arrête-toi et demande, dans ces cas :

- une décision de la section 12 de la spec est requise
- une mesure invalide une hypothèse structurante (schéma, stratégie GraphQL, mode d'auth)
- un budget du §4 est manqué d'un facteur supérieur à 2 et tu ne vois pas de cause claire
- il te manque un accès, un token ou un dépôt de test
- deux exigences de la spec se contredisent

Ne contourne pas un blocage par une solution de fortune non signalée. Écris le blocage dans `NOTES.md`, propose deux options avec leurs conséquences, et attends.

---

## Traces de session

Tiens un `NOTES.md` à la racine, en append seulement, avec pour chaque session :

- ce qui a été fait
- les mesures obtenues, avec le lien vers les logs bruts
- les décisions prises et leur justification en une ligne
- les blocages et les questions ouvertes
- ce qu'il faut faire ensuite

Ce fichier est ce qui donne à la session suivante le contexte que tu n'as plus.

---

## Clean code — règles impératives

**Interdiction totale des commentaires dans le code.** Aucun `//`, aucun `///`, aucun `/* */`, aucun `#`. Un commentaire est l'aveu d'un nom mal choisi ou d'une fonction qui fait trop de choses. Corrige la cause.

Si une contrainte externe doit être documentée (une bizarrerie de l'API GitHub, une limite non évidente), elle va à deux endroits, jamais dans un commentaire :
- un **test au nom explicite** qui la vérifie, par exemple `returns_304_without_consuming_quota_when_authenticated`
- une entrée dans `docs/github-api-constraints.md`

**Tout le code en anglais.** Noms de variables, de fonctions, de types, de modules, de fichiers, de branches, messages de commit, messages d'erreur techniques. Le français est réservé à `NOTES.md`, aux rapports et aux chaînes visibles par l'utilisateur.

**KISS.** La solution la plus simple qui satisfait l'exigence. Pas de généricité spéculative, pas de trait introduit pour un seul implémenteur sauf si la spec annonce le second (`EventSource` est le cas prévu). Pas de couche d'abstraction « au cas où ».

**DRY, avec discernement.** Trois occurrences avant de factoriser, pas deux. Une duplication ponctuelle coûte moins cher qu'une mauvaise abstraction.

**SOLID**, en pratique dans ce projet :
- une fonction fait une chose, un module a une raison de changer
- les crates dépendent d'abstractions, pas d'implémentations : `quay-sync` dépend de `EventSource`, pas de `PollingSource`
- pas d'interface obèse : préférer plusieurs traits étroits à un trait fourre-tout

**Nommage.** Le nom porte l'intention, pas le type ni l'implémentation. `stale_after` et non `timestamp2`. `RateGovernor` et non `HttpHelper`. Pas d'abréviation sauf celles du domaine (`pr`, `sha`, `etag`, `sso`).

**Fonctions.** Si tu ressens le besoin d'expliquer un bloc, extrais-le dans une fonction dont le nom est cette explication.

**Messages de commit** à l'impératif, en anglais, sujet sous 72 caractères, NE PAS INCLURE DE Co-Authored by. LE MESSAGE DE COMMIT NE DOIT PAS INCLURE DE NOM DE MODELE OU AUTRE.

Pas de dépendance ajoutée sans justification d'une ligne dans `NOTES.md`.

---

## Sécurité en développement

Deux verrous à implémenter dans le rate governor **avant** la première requête sortante :

**Mode lecture seule.** `QUAY_READONLY=1` par défaut en développement : toute méthode HTTP non sûre est rejetée avant l'envoi. Désactivation explicite, session par session, uniquement pour tester les mutations.

**Allowlist d'écriture.** Tant que `QUAY_WRITE_ALLOWLIST` est définie, aucune écriture ne part vers un dépôt absent de la liste. Les cibles de test sont des dépôts jetables sur le compte du développeur, jamais un dépôt d'équipe.

Ces deux verrous existent parce qu'une session de test qui poste un commentaire ou merge sur une vraie PR d'équipe est irréversible et embarrassante. Ils ne sont pas négociables et ne se contournent pas « juste pour vérifier un truc ».