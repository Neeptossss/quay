# Rapport de mesures — Quay

Ce fichier est généré par `cargo run -p xtask -- report` à partir des logs bruts de `measurements/raw/`. Il ne se modifie pas à la main.

- Généré le : 2026-09-04T16:13:00.071815Z
- Machine : aarch64-macos

## Budgets du §4

`MISSING` signifie qu'aucune mesure n'existe encore pour ce budget : il est rouge parce qu'un budget non mesuré est un budget non tenu, pas parce qu'il est dépassé.

| Budget | Réf. | Borne | Statistique | Instrument | Mesure | Verdict | Log brut |
|---|---|---|---|---|---|---|---|
| `cold_start_to_first_paint` (cold start to first painted list) | §4 | ≤ 400 ms | median | tauri trace, process start to paint | — | **MISSING** | — |
| `keystroke_to_pixel` (keystroke to updated pixel) | §4 | ≤ 16 ms | p99 | frontend instrumentation, performance.now() | — | **MISSING** | — |
| `cached_pull_request_navigation` (navigation to the next pull request from cache) | §4 | ≤ 50 ms | median | frontend instrumentation, performance.now() | — | **MISSING** | — |
| `inbox_query` (filtered inbox query on SQLite) | §4 | ≤ 5 ms | p99 | xtask measure j1b, reference dataset | 0.091 ms | **MET** | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-300open-0closed-5000comments-review_requested_exact-warm.csv` |
| `diff_open_two_thousand_lines` (opening a 2000 line diff) | §4 | ≤ 120 ms | median | bench, virtualised rendering | — | **MISSING** | — |
| `resident_memory_idle` (resident memory at rest, 20 repositories synchronised) | §4 | ≤ 250 MB | max | process measurement | — | **MISSING** | — |
| `background_cpu_idle` (cpu at rest, window in the background) | §4 | ≤ 0.5 % | mean | process measurement | — | **MISSING** | — |
| `github_quota_per_hour` (GitHub quota consumed in normal operation) | §4 | ≤ 1500 req/h | max | rate governor counter | — | **MISSING** | — |
| `cache_served_navigations` (navigations served from cache in under 50 ms) | §4 | ≥ 85 % | sliding window | preload_outcome | — | **MISSING** | — |

## Durabilité des écritures — §6 face au §7.3

- Mesuré le : 2026-09-04T15:53:50.296738Z
- Opération : optimistic approve: one transaction, one update and one queue insert
- Itérations : 2000

| Garantie | `synchronous` | `fullfsync` | p50 ms | p95 ms | p99 ms | max ms | Log brut |
|---|---|---|---|---|---|---|---|
| `process_crash_safe` | NORMAL | false | 0.015 | 0.027 | 0.042 | 2.056 | `measurements/raw/durability-2026-09-04T155350.296738Z-process_crash_safe.csv` |
| `power_loss_safe` | FULL | true | 3.996 | 4.674 | 5.965 | 10.819 | `measurements/raw/durability-2026-09-04T155350.296738Z-power_loss_safe.csv` |

## Critère de sortie du §9 — l'inbox réelle depuis SQLite

- Mesuré le : 2026-09-04T15:45:42.752704Z
- Machine : aarch64-macos

Ce jeu de données est **plus petit** que celui du §4, sur lequel les budgets sont définis : il ne remplace pas J1-b, il montre le produit sur des données réelles.

| Grandeur du jeu réel | Valeur |
|---|---|
| Dépôts synchronisés | 14 |
| Pull requests ouvertes | 34 |
| Threads de review | 23 |
| Commentaires de review | 23 |
| Reviews demandées à l'utilisateur | 24 |
| Dépôts distincts dans l'inbox | 8 |

| Requête | Cache | Lignes | p50 ms | p95 ms | p99 ms | max ms | Log brut |
|---|---|---|---|---|---|---|---|
| `review_requested_exact` | warm | 24 | 0.028 | 0.040 | 0.058 | 0.071 | `measurements/raw/inbox-real-2026-09-04T154542.752704Z-review_requested_exact-warm.csv` |
| `review_requested_exact` | cold | 24 | 0.046 | 0.047 | 0.049 | 0.068 | `measurements/raw/inbox-real-2026-09-04T154542.752704Z-review_requested_exact-cold.csv` |
| `open_with_unresolved_thread_count` | warm | 34 | 0.026 | 0.027 | 0.031 | 0.060 | `measurements/raw/inbox-real-2026-09-04T154542.752704Z-open_with_unresolved_thread_count-warm.csv` |
| `open_with_unresolved_thread_count` | cold | 34 | 0.041 | 0.042 | 0.045 | 0.051 | `measurements/raw/inbox-real-2026-09-04T154542.752704Z-open_with_unresolved_thread_count-cold.csv` |

## M0-1 — authentification acceptée par l'endpoint `/notifications`

- Mesuré le : 2026-09-04T15:02:32.414153Z
- Log brut : `measurements/raw/m0-1-2026-09-04T150232.414153Z.jsonl`

La mesure ne porte que sur le type de jeton réellement disponible. Les autres modes d'authentification restent non mesurés, ils ne sont pas déduits.

### `identity`

| Observation | Valeur |
|---|---|
| `granted_scopes` | `["audit_log","notifications","project","public_repo","read:org","read:user","repo","repo:invite","repo:status","repo_deployment","security_events","user","user:email","user:follow","workflow"]` |
| `login_resolved` | `true` |
| `organization_count` | `9` |
| `single_sign_on` | none |
| `token_kind` | classic personal access token |

### `notifications_probe`

| Observation | Valeur |
|---|---|
| `bytes` | `58704` |
| `capability` | `true` |
| `entries` | `12` |
| `etag_offered` | `true` |
| `last_modified_offered` | `true` |
| `message` | — |
| `poll_interval_seconds` | `60` |
| `status` | `200` |

### `revalidated_campaign`

| Observation | Valeur |
|---|---|
| `bytes` | `0` |
| `elapsed_ms` | `{"max":506.894125,"mean":418.6486957999999,"min":356.852125,"p50":418.418583,"p95":506.894125,"p99":506.894125,"samples":10}` |
| `quota_remaining_in_response_headers` | `[4968,4968,4968,4968,4968,4968,4968,4968,4968,4968]` |
| `rate_limit_endpoint_buckets_that_moved` | `{}` |
| `requests` | `10` |
| `statuses` | `[304,304,304,304,304,304,304,304,304,304]` |

### `plain_campaign`

| Observation | Valeur |
|---|---|
| `bytes` | `587040` |
| `elapsed_ms` | `{"max":477.3923339999999,"mean":391.5111292,"min":325.488375,"p50":387.719375,"p95":477.3923339999999,"p99":477.3923339999999,"samples":10}` |
| `quota_remaining_in_response_headers` | `[4967,4966,4965,4964,4963,4962,4961,4960,4959,4958]` |
| `rate_limit_endpoint_buckets_that_moved` | `{}` |
| `requests` | `10` |
| `statuses` | `[200,200,200,200,200,200,200,200,200,200]` |

## J1-a — point de rupture de la requête GraphQL de détail de pull request

- Mesuré le : 2026-09-04T14:14:17.385968Z
- Machine : aarch64-macos
- Log brut : `measurements/raw/j1a-2026-09-04T141417.385968Z.jsonl`

| Cible | Sélection | Fichiers | Threads | Mode | Pagination | Rép. | p50 ms | p95 ms | max ms | Octets | Coût | Nœuds | Fichiers rendus | Threads rendus | Tronqué | Erreurs |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `rust-lang/rust#161795` | files~5 | 5 | 3 | single_shot | small_pages | 7 | 719 | 864 | 864 | 3288 | 1 | 241 | 5/5 | 3/3 | non | 0 |
| `rust-lang/rust#161795` | files~5 | 5 | 3 | single_shot | medium_pages | 7 | 690 | 930 | 930 | 3289 | 1 | 1351 | 5/5 | 3/3 | non | 0 |
| `rust-lang/rust#161795` | files~5 | 5 | 3 | single_shot | max_pages | 7 | 866 | 968 | 968 | 3290 | 1 | 10201 | 5/5 | 3/3 | non | 0 |
| `rust-lang/rust#161795` | files~5 | 5 | 3 | paginated | files100_threads50_comments25 | 7 | 832 | 1050 | 1050 | 3039 | 2 | 1400 | 5/5 | 3/3 | non | 0 |
| `odoo/odoo#69930` | files~50 | 50 | 13 | single_shot | small_pages | 7 | 752 | 945 | 945 | 13087 | 1 | 241 | 20/50 | 13/13 | oui | 0 |
| `odoo/odoo#69930` | files~50 | 50 | 13 | single_shot | medium_pages | 7 | 945 | 1611 | 1611 | 15935 | 1 | 1351 | 50/50 | 13/13 | non | 0 |
| `odoo/odoo#69930` | files~50 | 50 | 13 | single_shot | max_pages | 7 | 819 | 1025 | 1025 | 15936 | 1 | 10201 | 50/50 | 13/13 | non | 0 |
| `odoo/odoo#69930` | files~50 | 50 | 13 | paginated | files100_threads50_comments25 | 7 | 1022 | 1526 | 1526 | 15696 | 2 | 1400 | 50/50 | 13/13 | non | 0 |
| `odoo/odoo#149028` | files~300 | 323 | 2 | single_shot | small_pages | 7 | 873 | 963 | 963 | 3342 | 1 | 241 | 20/323 | 2/2 | oui | 0 |
| `odoo/odoo#149028` | files~300 | 323 | 2 | single_shot | medium_pages | 7 | 736 | 842 | 842 | 6244 | 1 | 1351 | 50/323 | 2/2 | oui | 0 |
| `odoo/odoo#149028` | files~300 | 323 | 2 | single_shot | max_pages | 7 | 867 | 1146 | 1146 | 11247 | 1 | 10201 | 100/323 | 2/2 | oui | 0 |
| `odoo/odoo#149028` | files~300 | 323 | 2 | paginated | files100_threads50_comments25 | 7 | 2397 | 2676 | 2676 | 32724 | 5 | 1700 | 323/323 | 2/2 | non | 0 |
| `odoo/odoo#63177` | threads-max | 70 | 170 | single_shot | small_pages | 7 | 719 | 1253 | 1253 | 12076 | 1 | 241 | 20/70 | 20/170 | oui | 0 |
| `odoo/odoo#63177` | threads-max | 70 | 170 | single_shot | medium_pages | 7 | 1160 | 1705 | 1705 | 36682 | 1 | 1351 | 50/70 | 50/170 | oui | 0 |
| `odoo/odoo#63177` | threads-max | 70 | 170 | single_shot | max_pages | 7 | 1423 | 2234 | 2234 | 72608 | 1 | 10201 | 70/70 | 100/170 | oui | 0 |
| `odoo/odoo#63177` | threads-max | 70 | 170 | paginated | files100_threads50_comments25 | 7 | 3682 | 3827 | 3827 | 119490 | 5 | 5300 | 70/70 | 170/170 | non | 0 |

## J1-b — requête d'inbox sur dataset synthétique

- Mesuré le : 2026-09-04T15:16:30.145759Z
- Machine : aarch64-macos

Le budget `inbox_query` retient le p99 le plus défavorable des requêtes du schéma corrigé, dataset de référence, cache chaud. Le schéma `section6` est le §6 verbatim, conservé comme témoin de comparaison.

| Schéma | Dataset | Requête | Cache | Lignes | p50 ms | p95 ms | p99 ms | max ms | Échantillons | Log brut |
|---|---|---|---|---|---|---|---|---|---|---|
| section6 | 20repos-300open-0closed-5000comments | `open_across_tracked_repos` | warm | 50 | 0.031 | 0.035 | 0.041 | 0.128 | 10000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-300open-0closed-5000comments-open_across_tracked_repos-warm.csv` |
| section6 | 20repos-300open-0closed-5000comments | `open_across_tracked_repos` | cold | 50 | 0.077 | 0.097 | 0.107 | 0.163 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-300open-0closed-5000comments-open_across_tracked_repos-cold.csv` |
| section6 | 20repos-300open-0closed-5000comments | `open_with_unresolved_thread_count` | warm | 50 | 0.151 | 0.163 | 0.181 | 0.355 | 10000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-300open-0closed-5000comments-open_with_unresolved_thread_count-warm.csv` |
| section6 | 20repos-300open-0closed-5000comments | `open_with_unresolved_thread_count` | cold | 50 | 0.214 | 0.227 | 0.250 | 0.254 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-300open-0closed-5000comments-open_with_unresolved_thread_count-cold.csv` |
| section6 | 20repos-300open-0closed-5000comments | `review_requested_approximation` | warm | 50 | 0.045 | 0.049 | 0.057 | 0.136 | 10000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-300open-0closed-5000comments-review_requested_approximation-warm.csv` |
| section6 | 20repos-300open-0closed-5000comments | `review_requested_approximation` | cold | 50 | 0.125 | 0.143 | 0.169 | 0.179 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-300open-0closed-5000comments-review_requested_approximation-cold.csv` |
| section6 | 20repos-300open-0closed-5000comments | `open_filtered_by_common_title` | warm | 50 | 0.035 | 0.039 | 0.049 | 0.365 | 10000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-300open-0closed-5000comments-open_filtered_by_common_title-warm.csv` |
| section6 | 20repos-300open-0closed-5000comments | `open_filtered_by_common_title` | cold | 50 | 0.077 | 0.096 | 0.115 | 0.125 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-300open-0closed-5000comments-open_filtered_by_common_title-cold.csv` |
| section6 | 20repos-300open-0closed-5000comments | `open_filtered_by_rare_title` | warm | 0 | 0.049 | 0.058 | 0.112 | 1.593 | 10000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-300open-0closed-5000comments-open_filtered_by_rare_title-warm.csv` |
| section6 | 20repos-300open-0closed-5000comments | `open_filtered_by_rare_title` | cold | 0 | 0.200 | 0.220 | 0.255 | 0.286 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-300open-0closed-5000comments-open_filtered_by_rare_title-cold.csv` |
| section6 | 20repos-3000open-0closed-50000comments | `open_across_tracked_repos` | warm | 50 | 0.031 | 0.035 | 0.043 | 0.126 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-3000open-0closed-50000comments-open_across_tracked_repos-warm.csv` |
| section6 | 20repos-3000open-0closed-50000comments | `open_across_tracked_repos` | cold | 50 | 0.071 | 0.079 | 0.085 | 0.106 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-3000open-0closed-50000comments-open_across_tracked_repos-cold.csv` |
| section6 | 20repos-3000open-0closed-50000comments | `open_with_unresolved_thread_count` | warm | 50 | 11.189 | 11.437 | 11.668 | 15.092 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-3000open-0closed-50000comments-open_with_unresolved_thread_count-warm.csv` |
| section6 | 20repos-3000open-0closed-50000comments | `open_with_unresolved_thread_count` | cold | 50 | 11.446 | 11.651 | 11.693 | 11.749 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-3000open-0closed-50000comments-open_with_unresolved_thread_count-cold.csv` |
| section6 | 20repos-3000open-0closed-50000comments | `review_requested_approximation` | warm | 50 | 0.045 | 0.049 | 0.059 | 0.195 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-3000open-0closed-50000comments-review_requested_approximation-warm.csv` |
| section6 | 20repos-3000open-0closed-50000comments | `review_requested_approximation` | cold | 50 | 0.123 | 0.142 | 0.159 | 0.210 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-3000open-0closed-50000comments-review_requested_approximation-cold.csv` |
| section6 | 20repos-3000open-0closed-50000comments | `open_filtered_by_common_title` | warm | 50 | 0.034 | 0.037 | 0.042 | 0.142 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-3000open-0closed-50000comments-open_filtered_by_common_title-warm.csv` |
| section6 | 20repos-3000open-0closed-50000comments | `open_filtered_by_common_title` | cold | 50 | 0.077 | 0.085 | 0.097 | 0.107 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-3000open-0closed-50000comments-open_filtered_by_common_title-cold.csv` |
| section6 | 20repos-3000open-0closed-50000comments | `open_filtered_by_rare_title` | warm | 0 | 1.730 | 2.015 | 2.166 | 2.583 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-3000open-0closed-50000comments-open_filtered_by_rare_title-warm.csv` |
| section6 | 20repos-3000open-0closed-50000comments | `open_filtered_by_rare_title` | cold | 0 | 1.896 | 2.164 | 2.338 | 2.367 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-3000open-0closed-50000comments-open_filtered_by_rare_title-cold.csv` |
| section6 | 20repos-30000open-0closed-500000comments | `open_across_tracked_repos` | warm | 50 | 0.032 | 0.035 | 0.051 | 0.119 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-30000open-0closed-500000comments-open_across_tracked_repos-warm.csv` |
| section6 | 20repos-30000open-0closed-500000comments | `open_across_tracked_repos` | cold | 50 | 0.081 | 0.109 | 0.128 | 0.151 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-30000open-0closed-500000comments-open_across_tracked_repos-cold.csv` |
| section6 | 20repos-30000open-0closed-500000comments | `open_with_unresolved_thread_count` | warm | 50 | 183.475 | 192.217 | 203.771 | 298.846 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-30000open-0closed-500000comments-open_with_unresolved_thread_count-warm.csv` |
| section6 | 20repos-30000open-0closed-500000comments | `open_with_unresolved_thread_count` | cold | 50 | 184.367 | 197.530 | 210.502 | 237.557 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-30000open-0closed-500000comments-open_with_unresolved_thread_count-cold.csv` |
| section6 | 20repos-30000open-0closed-500000comments | `review_requested_approximation` | warm | 50 | 0.045 | 0.053 | 0.061 | 0.105 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-30000open-0closed-500000comments-review_requested_approximation-warm.csv` |
| section6 | 20repos-30000open-0closed-500000comments | `review_requested_approximation` | cold | 50 | 0.121 | 0.131 | 0.147 | 0.169 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-30000open-0closed-500000comments-review_requested_approximation-cold.csv` |
| section6 | 20repos-30000open-0closed-500000comments | `open_filtered_by_common_title` | warm | 50 | 0.034 | 0.039 | 0.047 | 0.134 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-30000open-0closed-500000comments-open_filtered_by_common_title-warm.csv` |
| section6 | 20repos-30000open-0closed-500000comments | `open_filtered_by_common_title` | cold | 50 | 0.078 | 0.088 | 0.107 | 0.108 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-30000open-0closed-500000comments-open_filtered_by_common_title-cold.csv` |
| section6 | 20repos-30000open-0closed-500000comments | `open_filtered_by_rare_title` | warm | 0 | 19.502 | 20.184 | 21.553 | 24.540 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-30000open-0closed-500000comments-open_filtered_by_rare_title-warm.csv` |
| section6 | 20repos-30000open-0closed-500000comments | `open_filtered_by_rare_title` | cold | 0 | 19.475 | 20.068 | 20.147 | 20.495 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-section6-20repos-30000open-0closed-500000comments-open_filtered_by_rare_title-cold.csv` |
| corrected | 20repos-300open-0closed-5000comments | `open_across_tracked_repos` | warm | 50 | 0.029 | 0.035 | 0.041 | 0.077 | 10000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-300open-0closed-5000comments-open_across_tracked_repos-warm.csv` |
| corrected | 20repos-300open-0closed-5000comments | `open_across_tracked_repos` | cold | 50 | 0.046 | 0.054 | 0.061 | 0.065 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-300open-0closed-5000comments-open_across_tracked_repos-cold.csv` |
| corrected | 20repos-300open-0closed-5000comments | `open_with_unresolved_thread_count` | warm | 50 | 0.041 | 0.048 | 0.061 | 0.241 | 10000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-300open-0closed-5000comments-open_with_unresolved_thread_count-warm.csv` |
| corrected | 20repos-300open-0closed-5000comments | `open_with_unresolved_thread_count` | cold | 50 | 0.062 | 0.075 | 0.088 | 0.097 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-300open-0closed-5000comments-open_with_unresolved_thread_count-cold.csv` |
| corrected | 20repos-300open-0closed-5000comments | `review_requested_approximation` | warm | 50 | 0.039 | 0.047 | 0.055 | 0.168 | 10000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-300open-0closed-5000comments-review_requested_approximation-warm.csv` |
| corrected | 20repos-300open-0closed-5000comments | `review_requested_approximation` | cold | 50 | 0.061 | 0.069 | 0.077 | 0.086 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-300open-0closed-5000comments-review_requested_approximation-cold.csv` |
| corrected | 20repos-300open-0closed-5000comments | `open_filtered_by_common_title` | warm | 50 | 0.032 | 0.038 | 0.046 | 0.156 | 10000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-300open-0closed-5000comments-open_filtered_by_common_title-warm.csv` |
| corrected | 20repos-300open-0closed-5000comments | `open_filtered_by_common_title` | cold | 50 | 0.050 | 0.062 | 0.071 | 0.072 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-300open-0closed-5000comments-open_filtered_by_common_title-cold.csv` |
| corrected | 20repos-300open-0closed-5000comments | `open_filtered_by_rare_title` | warm | 0 | 0.033 | 0.039 | 0.046 | 0.174 | 10000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-300open-0closed-5000comments-open_filtered_by_rare_title-warm.csv` |
| corrected | 20repos-300open-0closed-5000comments | `open_filtered_by_rare_title` | cold | 0 | 0.054 | 0.069 | 0.080 | 0.086 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-300open-0closed-5000comments-open_filtered_by_rare_title-cold.csv` |
| corrected | 20repos-300open-0closed-5000comments | `review_requested_exact` | warm | 50 | 0.067 | 0.078 | 0.091 | 0.216 | 10000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-300open-0closed-5000comments-review_requested_exact-warm.csv` |
| corrected | 20repos-300open-0closed-5000comments | `review_requested_exact` | cold | 50 | 0.099 | 0.112 | 0.127 | 0.136 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-300open-0closed-5000comments-review_requested_exact-cold.csv` |
| corrected | 20repos-3000open-0closed-50000comments | `open_across_tracked_repos` | warm | 50 | 0.029 | 0.035 | 0.040 | 0.057 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-3000open-0closed-50000comments-open_across_tracked_repos-warm.csv` |
| corrected | 20repos-3000open-0closed-50000comments | `open_across_tracked_repos` | cold | 50 | 0.046 | 0.052 | 0.064 | 0.066 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-3000open-0closed-50000comments-open_across_tracked_repos-cold.csv` |
| corrected | 20repos-3000open-0closed-50000comments | `open_with_unresolved_thread_count` | warm | 50 | 0.042 | 0.048 | 0.059 | 0.174 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-3000open-0closed-50000comments-open_with_unresolved_thread_count-warm.csv` |
| corrected | 20repos-3000open-0closed-50000comments | `open_with_unresolved_thread_count` | cold | 50 | 0.062 | 0.082 | 0.090 | 0.105 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-3000open-0closed-50000comments-open_with_unresolved_thread_count-cold.csv` |
| corrected | 20repos-3000open-0closed-50000comments | `review_requested_approximation` | warm | 50 | 0.039 | 0.045 | 0.053 | 0.068 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-3000open-0closed-50000comments-review_requested_approximation-warm.csv` |
| corrected | 20repos-3000open-0closed-50000comments | `review_requested_approximation` | cold | 50 | 0.060 | 0.068 | 0.079 | 0.085 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-3000open-0closed-50000comments-review_requested_approximation-cold.csv` |
| corrected | 20repos-3000open-0closed-50000comments | `open_filtered_by_common_title` | warm | 50 | 0.032 | 0.037 | 0.043 | 0.064 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-3000open-0closed-50000comments-open_filtered_by_common_title-warm.csv` |
| corrected | 20repos-3000open-0closed-50000comments | `open_filtered_by_common_title` | cold | 50 | 0.050 | 0.058 | 0.070 | 0.072 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-3000open-0closed-50000comments-open_filtered_by_common_title-cold.csv` |
| corrected | 20repos-3000open-0closed-50000comments | `open_filtered_by_rare_title` | warm | 0 | 0.227 | 0.254 | 0.281 | 0.464 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-3000open-0closed-50000comments-open_filtered_by_rare_title-warm.csv` |
| corrected | 20repos-3000open-0closed-50000comments | `open_filtered_by_rare_title` | cold | 0 | 0.309 | 0.333 | 0.377 | 0.409 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-3000open-0closed-50000comments-open_filtered_by_rare_title-cold.csv` |
| corrected | 20repos-3000open-0closed-50000comments | `review_requested_exact` | warm | 50 | 0.075 | 0.087 | 0.114 | 0.433 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-3000open-0closed-50000comments-review_requested_exact-warm.csv` |
| corrected | 20repos-3000open-0closed-50000comments | `review_requested_exact` | cold | 50 | 0.104 | 0.122 | 0.149 | 0.162 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-3000open-0closed-50000comments-review_requested_exact-cold.csv` |
| corrected | 20repos-30000open-0closed-500000comments | `open_across_tracked_repos` | warm | 50 | 0.030 | 0.035 | 0.043 | 0.179 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-30000open-0closed-500000comments-open_across_tracked_repos-warm.csv` |
| corrected | 20repos-30000open-0closed-500000comments | `open_across_tracked_repos` | cold | 50 | 0.048 | 0.065 | 0.075 | 0.077 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-30000open-0closed-500000comments-open_across_tracked_repos-cold.csv` |
| corrected | 20repos-30000open-0closed-500000comments | `open_with_unresolved_thread_count` | warm | 50 | 0.044 | 0.050 | 0.068 | 0.176 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-30000open-0closed-500000comments-open_with_unresolved_thread_count-warm.csv` |
| corrected | 20repos-30000open-0closed-500000comments | `open_with_unresolved_thread_count` | cold | 50 | 0.066 | 0.093 | 0.112 | 0.163 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-30000open-0closed-500000comments-open_with_unresolved_thread_count-cold.csv` |
| corrected | 20repos-30000open-0closed-500000comments | `review_requested_approximation` | warm | 50 | 0.040 | 0.046 | 0.057 | 0.170 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-30000open-0closed-500000comments-review_requested_approximation-warm.csv` |
| corrected | 20repos-30000open-0closed-500000comments | `review_requested_approximation` | cold | 50 | 0.062 | 0.071 | 0.077 | 0.080 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-30000open-0closed-500000comments-review_requested_approximation-cold.csv` |
| corrected | 20repos-30000open-0closed-500000comments | `open_filtered_by_common_title` | warm | 50 | 0.032 | 0.039 | 0.044 | 0.082 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-30000open-0closed-500000comments-open_filtered_by_common_title-warm.csv` |
| corrected | 20repos-30000open-0closed-500000comments | `open_filtered_by_common_title` | cold | 50 | 0.051 | 0.058 | 0.065 | 0.072 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-30000open-0closed-500000comments-open_filtered_by_common_title-cold.csv` |
| corrected | 20repos-30000open-0closed-500000comments | `open_filtered_by_rare_title` | warm | 0 | 3.061 | 3.361 | 3.542 | 4.163 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-30000open-0closed-500000comments-open_filtered_by_rare_title-warm.csv` |
| corrected | 20repos-30000open-0closed-500000comments | `open_filtered_by_rare_title` | cold | 0 | 3.134 | 3.471 | 3.591 | 3.610 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-30000open-0closed-500000comments-open_filtered_by_rare_title-cold.csv` |
| corrected | 20repos-30000open-0closed-500000comments | `review_requested_exact` | warm | 50 | 0.081 | 0.094 | 0.108 | 0.186 | 2000 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-30000open-0closed-500000comments-review_requested_exact-warm.csv` |
| corrected | 20repos-30000open-0closed-500000comments | `review_requested_exact` | cold | 50 | 0.115 | 0.129 | 0.134 | 0.158 | 100 | `measurements/raw/j1b-2026-09-04T151630.145759Z-corrected-20repos-30000open-0closed-500000comments-review_requested_exact-cold.csv` |

### Plans d'exécution SQLite

- `section6/open_across_tracked_repos` : SEARCH pr USING INDEX idx_pr_inbox (state=?) ; BLOOM FILTER ON r (id=?) ; SEARCH r USING INTEGER PRIMARY KEY (rowid=?)
- `section6/open_with_unresolved_thread_count` : SEARCH pr USING INDEX idx_pr_inbox (state=?) ; BLOOM FILTER ON r (id=?) ; SEARCH r USING INTEGER PRIMARY KEY (rowid=?) ; CORRELATED SCALAR SUBQUERY 1 ; BLOOM FILTER ON t (pr_id=? AND is_resolved=?) ; SEARCH t USING AUTOMATIC PARTIAL COVERING INDEX (pr_id=? AND is_resolved=?)
- `section6/review_requested_approximation` : SEARCH pr USING INDEX idx_pr_inbox (state=?) ; BLOOM FILTER ON r (id=?) ; SEARCH r USING INTEGER PRIMARY KEY (rowid=?)
- `section6/open_filtered_by_common_title` : SEARCH pr USING INDEX idx_pr_inbox (state=?) ; BLOOM FILTER ON r (id=?) ; SEARCH r USING INTEGER PRIMARY KEY (rowid=?)
- `section6/open_filtered_by_rare_title` : SEARCH pr USING INDEX idx_pr_inbox (state=?) ; BLOOM FILTER ON r (id=?) ; SEARCH r USING INTEGER PRIMARY KEY (rowid=?)
- `corrected/open_across_tracked_repos` : SEARCH pr USING INDEX idx_pr_inbox (state=?) ; BLOOM FILTER ON r (id=?) ; SEARCH r USING INTEGER PRIMARY KEY (rowid=?)
- `corrected/open_with_unresolved_thread_count` : SEARCH pr USING INDEX idx_pr_inbox (state=?) ; BLOOM FILTER ON r (id=?) ; SEARCH r USING INTEGER PRIMARY KEY (rowid=?) ; CORRELATED SCALAR SUBQUERY 1 ; SEARCH t USING COVERING INDEX idx_thread_by_pull_request (pr_id=? AND is_resolved=?)
- `corrected/review_requested_approximation` : SEARCH pr USING INDEX idx_pr_inbox (state=?) ; BLOOM FILTER ON r (id=?) ; SEARCH r USING INTEGER PRIMARY KEY (rowid=?)
- `corrected/open_filtered_by_common_title` : SEARCH pr USING INDEX idx_pr_inbox (state=?) ; BLOOM FILTER ON r (id=?) ; SEARCH r USING INTEGER PRIMARY KEY (rowid=?)
- `corrected/open_filtered_by_rare_title` : SEARCH pr USING INDEX idx_pr_inbox (state=?) ; BLOOM FILTER ON r (id=?) ; SEARCH r USING INTEGER PRIMARY KEY (rowid=?)
- `corrected/review_requested_exact` : SEARCH pr USING INDEX idx_pr_inbox (state=?) ; BLOOM FILTER ON r (id=?) ; SEARCH r USING INTEGER PRIMARY KEY (rowid=?) ; SEARCH rr EXISTS USING COVERING INDEX idx_review_request_by_reviewer (reviewer=? AND pr_id=?) ; CORRELATED SCALAR SUBQUERY 1 ; SEARCH t USING COVERING INDEX idx_thread_by_pull_request (pr_id=? AND is_resolved=?)

