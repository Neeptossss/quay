# Contraintes SQLite

Mêmes règles que `github-api-constraints.md` : une contrainte non évidente se documente ici et se
vérifie par un test au nom explicite, jamais par un commentaire dans le code.

| Contrainte | Conséquence pour Quay | Test qui la vérifie |
|---|---|---|
| SQLite n'indexe pas automatiquement les clés étrangères | Une sous-requête corrélée sur `review_thread(pr_id)` fait construire un index automatique à chaque exécution. Mesuré à 267 ms sur 30 000 PR, ramené à 0,059 ms par un index explicite | `the_corrected_schema_indexes_every_foreign_key_the_inbox_walks` |
| `ON DELETE CASCADE` parcourt la table enfant entière sans index sur la clé étrangère | Chaque suppression de PR balaie `review_thread` et `review_comment` | `the_corrected_schema_indexes_every_foreign_key_the_inbox_walks` |
| Une colonne BLOB large fait déborder les pages de la table et ralentit tout balayage, même quand la colonne n'est pas sélectionnée | `pull_request.raw` déplacé dans `pull_request_payload`, facteur 5,7 sur les balayages sélectifs | `the_pull_request_row_carries_the_raw_payload_only_in_the_baseline_schema` |
| `PRAGMA synchronous = NORMAL` en mode WAL survit à un crash de processus mais pas à une coupure d'alimentation | À trancher explicitement au regard de l'invariant du §7.3 | non tranché, cf. `NOTES.md` |
| `WITHOUT ROWID` supprime l'index rowid redondant sur une table entièrement définie par sa clé primaire composite | Utilisé pour `review_request` | `the_corrected_schema_adds_the_review_request_and_payload_tables` |
| `PRAGMA synchronous = FULL` seul ne garantit pas la durabilité sur macOS : `fsync()` y rend la main avant que le disque ait écrit. Il faut `PRAGMA fullfsync = ON` | Mesuré : 0,086 ms de p99 avec `FULL` seul contre 5,965 ms avec le vidage réel du cache disque. Le premier chiffre aurait soutenu une promesse de durabilité fausse | `asking_for_power_loss_safety_turns_on_both_pragmas_it_needs` |
| Une vraie durabilité à la coupure de courant coûte 5,965 ms de p99 sur une mutation optimiste, au-dessus du budget de 5 ms du §5.4 et du §7.3 | Le défaut est `ProcessCrashSafe`, cf. `NOTES.md` | `the_default_store_is_safe_against_a_process_crash_not_against_a_power_loss` |
| `ALTER TABLE ... ADD COLUMN` est non destructif et accepté par SQLite | La sélection d'organisation est une colonne de `account`, pas une table clé-valeur générique dont un seul usage aurait justifié l'existence | `a_database_stopped_at_the_first_version_is_carried_to_the_latest` |
