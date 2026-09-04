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
