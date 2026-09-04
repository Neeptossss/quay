# Contraintes de l'API GitHub

Ce fichier recense les bizarreries et limites non évidentes de l'API GitHub sur lesquelles le code
s'appuie. Chaque entrée doit être vérifiée par un test au nom explicite. Un commentaire dans le code
n'est jamais le bon endroit pour ces informations.

| Contrainte | Source | Test qui la vérifie |
|---|---|---|
| Une réponse 304 authentifiée ne décompte pas du rate limit primaire | §3.2 de la spec | `a_304_costs_no_point_while_a_200_costs_one` |
| La limite secondaire est de 100 requêtes concurrentes ; on se plafonne à 8 | §3.3 de la spec | `the_default_concurrency_ceiling_is_eight_permits_not_one_hundred` |
| Un dépassement renvoie 403 ou 429, éventuellement avec `Retry-After` | §3.3 de la spec | `a_403_without_retry_after_is_retried_and_finally_reported_as_throttled`, `a_429_with_retry_after_zero_is_retried_immediately_and_can_succeed` |
| Le barème de points est GET = 1, mutation = 5 | §3.3 de la spec | `a_304_costs_no_point_while_a_200_costs_one` |
| Sous 20 % de quota restant, le client bascule en mode dégradé | §7.2 de la spec | `a_quota_below_twenty_percent_switches_the_governor_to_degraded` |
| Une requête GraphQL passe par POST, donc le verrou de lecture seule ne peut pas se fonder sur la seule méthode HTTP | mesuré en J1-a | `read_only_mode_still_allows_a_graphql_query_over_post`, `a_mutation_disguised_as_a_query_is_rejected_even_outside_read_only_mode` |
| L'endpoint `/notifications` fonctionne avec un PAT classique, annonce `X-Poll-Interval: 60` et propose à la fois `ETag` et `Last-Modified` | mesuré en M0-1 | `measurements/summary/m0-1.json`, étape `notifications_probe` |
| Une requête conditionnelle authentifiée qui renvoie 304 ne décrémente pas `x-ratelimit-remaining` ; dix 304 consécutifs laissent le compteur figé, dix 200 le décrémentent de 1 chacun | mesuré en M0-1 | `measurements/summary/m0-1.json`, étapes `revalidated_campaign` et `plain_campaign` |
| L'endpoint `/rate_limit` n'a reflété aucune des vingt requêtes mesurées, alors que les en-têtes de réponse les comptaient une par une. Seuls les en-têtes de réponse sont un instrument fiable | mesuré en M0-1 | `the_governor_never_polls_the_rate_limit_endpoint_by_itself` |
